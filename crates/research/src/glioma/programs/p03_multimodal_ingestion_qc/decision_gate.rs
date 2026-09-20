//! Uncertainty-aware endpoint decision gating for autonomous preclinical glioma workflows.
//!
//! A research engine needs a typed handoff between measurement analysis and the next workflow
//! stage.  This feature turns eligible multimodal endpoint estimates into a bounded interval and
//! evaluates that interval against an investigator-declared research threshold.  It never
//! converts a threshold into a clinical decision: ambiguous, contradictory, missing, and
//! low-confidence evidence remain explicit and route back to acquisition or experiment design.

use super::evidence_fusion::EndpointEvidence;
use crate::glioma_engine::{GliomaModality, GliomaModelSystem};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeSet, HashMap};
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P03-F18";
pub const OUTPUT_SCHEMA: &str = "GliomaMultimodalDecisionGate1@1";
pub const MAX_MODALITIES: usize = 64;
pub const MAX_ABS_VALUE_MILLI: u64 = 1_000_000_000;
pub const MAX_UNCERTAINTY_MILLI: u64 = 1_000_000_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DecisionDirection {
    HigherSupports,
    LowerSupports,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MultimodalDecisionGateRequest {
    pub objective: String,
    pub endpoint_id: String,
    pub study_id: String,
    pub model_system: GliomaModelSystem,
    pub required_modalities: Vec<GliomaModality>,
    pub observations: Vec<EndpointEvidence>,
    pub direction: DecisionDirection,
    pub threshold_milli: i64,
    pub min_modalities: usize,
    pub min_reliability_milli: u16,
    pub max_uncertainty_milli: u64,
    pub max_interval_width_milli: u64,
    pub min_margin_milli: u64,
    pub min_confidence_milli: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MultimodalDecisionGateDisposition {
    Supports,
    DoesNotSupport,
    Indeterminate,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DecisionGateModalityObservation {
    pub modality: GliomaModality,
    pub eligible: bool,
    pub weight_milli: u64,
    pub value_milli: Option<i64>,
    pub signed_margin_milli: Option<i64>,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MultimodalDecisionGateAnalysis {
    pub feature_id: String,
    pub output_schema: String,
    pub objective: String,
    pub endpoint_id: String,
    pub study_id: String,
    pub model_system: GliomaModelSystem,
    pub direction: DecisionDirection,
    pub threshold_milli: i64,
    pub modality_order: Vec<GliomaModality>,
    pub observations: Vec<DecisionGateModalityObservation>,
    pub baseline_value_milli: Option<i64>,
    pub lower_bound_milli: Option<i64>,
    pub upper_bound_milli: Option<i64>,
    pub interval_width_milli: Option<u64>,
    pub threshold_margin_milli: Option<u64>,
    pub confidence_milli: u16,
    pub eligible_modality_order: Vec<GliomaModality>,
    pub missing_modality_order: Vec<GliomaModality>,
    pub contradictory_modality_order: Vec<GliomaModality>,
    pub negative_evidence: Vec<String>,
    pub uncertainty: Vec<String>,
    pub disposition: MultimodalDecisionGateDisposition,
    pub next_action: String,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum MultimodalDecisionGateError {
    #[error("multimodal decision-gate request is invalid: {0}")]
    InvalidRequest(String),
    #[error("multimodal decision-gate input is invalid: {0}")]
    InvalidInput(String),
    #[error("multimodal decision-gate output is invalid: {0}")]
    InvalidOutput(String),
    #[error("multimodal decision-gate digest failed: {0}")]
    Digest(String),
}

fn canonical<T: Ord>(values: &[T]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn abs_difference(left: i64, right: i64) -> u64 {
    (i128::from(left) - i128::from(right)).unsigned_abs() as u64
}

fn clamp_value(value: i128) -> i64 {
    value.clamp(-(MAX_ABS_VALUE_MILLI as i128), MAX_ABS_VALUE_MILLI as i128) as i64
}

fn weight(observation: &EndpointEvidence) -> u64 {
    let numerator = u128::from(observation.reliability_milli)
        * u128::from(observation.quality_milli)
        * u128::from(observation.replicate_count)
        * 1_000;
    (numerator / (u128::from(observation.uncertainty_milli) + 1)) as u64
}

fn aggregate(observations: &[&EndpointEvidence]) -> Option<i64> {
    let mut numerator = 0i128;
    let mut denominator = 0u128;
    for observation in observations {
        let current_weight = weight(observation);
        if current_weight == 0 {
            continue;
        }
        numerator += i128::from(observation.value_milli) * i128::from(current_weight);
        denominator += u128::from(current_weight);
    }
    if denominator == 0 {
        None
    } else {
        Some(
            (numerator / i128::try_from(denominator).ok()?)
                .clamp(-(MAX_ABS_VALUE_MILLI as i128), MAX_ABS_VALUE_MILLI as i128)
                as i64,
        )
    }
}

fn supports(direction: DecisionDirection, value: i64, threshold: i64) -> bool {
    match direction {
        DecisionDirection::HigherSupports => value >= threshold,
        DecisionDirection::LowerSupports => value <= threshold,
    }
}

fn signed_margin(direction: DecisionDirection, value: i64, threshold: i64) -> i64 {
    match direction {
        DecisionDirection::HigherSupports => value.saturating_sub(threshold),
        DecisionDirection::LowerSupports => threshold.saturating_sub(value),
    }
}

fn digest_input(output: &MultimodalDecisionGateAnalysis) -> serde_json::Value {
    serde_json::json!({
        "feature_id": output.feature_id,
        "output_schema": output.output_schema,
        "objective": output.objective,
        "endpoint_id": output.endpoint_id,
        "study_id": output.study_id,
        "model_system": output.model_system,
        "direction": output.direction,
        "threshold_milli": output.threshold_milli,
        "modality_order": output.modality_order,
        "observations": output.observations,
        "baseline_value_milli": output.baseline_value_milli,
        "lower_bound_milli": output.lower_bound_milli,
        "upper_bound_milli": output.upper_bound_milli,
        "interval_width_milli": output.interval_width_milli,
        "threshold_margin_milli": output.threshold_margin_milli,
        "confidence_milli": output.confidence_milli,
        "eligible_modality_order": output.eligible_modality_order,
        "missing_modality_order": output.missing_modality_order,
        "contradictory_modality_order": output.contradictory_modality_order,
        "negative_evidence": output.negative_evidence,
        "uncertainty": output.uncertainty,
        "disposition": output.disposition,
        "next_action": output.next_action,
    })
}

fn validate_request(
    request: &MultimodalDecisionGateRequest,
) -> Result<(), MultimodalDecisionGateError> {
    if request.objective.trim().is_empty()
        || request.endpoint_id.trim().is_empty()
        || request.study_id.trim().is_empty()
        || request.required_modalities.len() > MAX_MODALITIES
        || request.observations.is_empty()
        || request.observations.len() > MAX_MODALITIES
        || request.min_modalities == 0
        || request.min_modalities > request.observations.len()
        || request.min_reliability_milli > 1_000
        || request.max_uncertainty_milli > MAX_UNCERTAINTY_MILLI
        || request.max_interval_width_milli == 0
        || request.max_interval_width_milli > MAX_ABS_VALUE_MILLI
        || request.min_margin_milli > MAX_ABS_VALUE_MILLI
        || request.min_confidence_milli > 1_000
    {
        return Err(MultimodalDecisionGateError::InvalidRequest(
            "endpoint/study identity, bounded observations, modality floor, interval gate, and confidence bound are required".into(),
        ));
    }
    if !canonical(&request.required_modalities) {
        return Err(MultimodalDecisionGateError::InvalidRequest(
            "required modalities must be unique and canonical".into(),
        ));
    }
    let required = request
        .required_modalities
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    let mut seen = BTreeSet::new();
    for observation in &request.observations {
        if !seen.insert(observation.modality)
            || (!required.is_empty() && !required.contains(&observation.modality))
            || observation.value_milli.unsigned_abs() > MAX_ABS_VALUE_MILLI
            || observation.uncertainty_milli > MAX_UNCERTAINTY_MILLI
            || observation.reliability_milli > 1_000
            || observation.quality_milli > 1_000
            || observation.replicate_count == 0
        {
            return Err(MultimodalDecisionGateError::InvalidInput(
                "observations require unique declared modalities, bounded values/uncertainty/quality/reliability, and positive replicate counts".into(),
            ));
        }
    }
    Ok(())
}

fn validate_output(
    output: &MultimodalDecisionGateAnalysis,
) -> Result<(), MultimodalDecisionGateError> {
    if output.feature_id != FEATURE_ID
        || output.output_schema != OUTPUT_SCHEMA
        || output.objective.trim().is_empty()
        || output.endpoint_id.trim().is_empty()
        || output.study_id.trim().is_empty()
        || !canonical(&output.modality_order)
        || output.observations.len() != output.modality_order.len()
        || output
            .observations
            .windows(2)
            .any(|pair| pair[0].modality >= pair[1].modality)
        || !canonical(&output.eligible_modality_order)
        || !canonical(&output.missing_modality_order)
        || !canonical(&output.contradictory_modality_order)
        || !canonical(&output.negative_evidence)
        || !canonical(&output.uncertainty)
        || output.confidence_milli > 1_000
        || output
            .observations
            .iter()
            .any(|observation| observation.reason.trim().is_empty())
    {
        return Err(MultimodalDecisionGateError::InvalidOutput(
            "identity, modality ordering, cardinality, evidence ordering, or confidence invariants are invalid".into(),
        ));
    }
    let expected = ContentHash::of_value(&digest_input(output))
        .map_err(|error| MultimodalDecisionGateError::Digest(error.to_string()))?;
    if expected != output.digest {
        return Err(MultimodalDecisionGateError::InvalidOutput(
            "digest is not bound to multimodal decision-gate output".into(),
        ));
    }
    Ok(())
}

impl MultimodalDecisionGateAnalysis {
    pub fn validate(&self) -> Result<(), MultimodalDecisionGateError> {
        validate_output(self)
    }
}

/// Evaluate a de-identified preclinical endpoint against a declared research threshold.
pub fn analyze_glioma_multimodal_decision_gate(
    request: &MultimodalDecisionGateRequest,
) -> Result<MultimodalDecisionGateAnalysis, MultimodalDecisionGateError> {
    validate_request(request)?;
    let required = request
        .required_modalities
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    let mut by_modality = HashMap::new();
    for observation in &request.observations {
        by_modality.insert(observation.modality, observation);
    }
    let observed_modalities = by_modality.keys().copied().collect::<BTreeSet<_>>();
    let modality_order = required
        .union(&observed_modalities)
        .copied()
        .collect::<Vec<_>>();
    let eligible = request
        .observations
        .iter()
        .filter(|observation| {
            observation.reliability_milli >= request.min_reliability_milli
                && observation.quality_milli >= request.min_reliability_milli
                && observation.uncertainty_milli <= request.max_uncertainty_milli
                && weight(observation) > 0
        })
        .collect::<Vec<_>>();
    let eligible_set = eligible.iter().map(|o| o.modality).collect::<BTreeSet<_>>();
    let baseline = aggregate(&eligible);
    let mut negative = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    let mut modality_observations = Vec::with_capacity(modality_order.len());
    let mut contradictory = BTreeSet::new();

    for modality in &modality_order {
        let Some(observation) = by_modality.get(modality) else {
            negative.insert(format!("required-modality-missing:{modality:?}"));
            modality_observations.push(DecisionGateModalityObservation {
                modality: *modality,
                eligible: false,
                weight_milli: 0,
                value_milli: None,
                signed_margin_milli: None,
                reason: "required observation is absent".into(),
            });
            continue;
        };
        let eligible_for_gate = eligible_set.contains(modality);
        let margin = signed_margin(
            request.direction,
            observation.value_milli,
            request.threshold_milli,
        );
        if eligible_for_gate
            && (margin < 0)
                != baseline.is_some_and(|value| {
                    signed_margin(request.direction, value, request.threshold_milli) < 0
                })
        {
            contradictory.insert(*modality);
            uncertainty.insert(format!("modality-disagrees-with-aggregate:{modality:?}"));
        }
        if !eligible_for_gate {
            negative.insert(format!("ineligible-modality:{modality:?}"));
        }
        modality_observations.push(DecisionGateModalityObservation {
            modality: *modality,
            eligible: eligible_for_gate,
            weight_milli: weight(observation),
            value_milli: Some(observation.value_milli),
            signed_margin_milli: Some(margin),
            reason: if eligible_for_gate {
                "measurement admitted to threshold interval".into()
            } else {
                "reliability, quality, uncertainty, or weight gate failed".into()
            },
        });
    }

    let (lower, upper, interval_width, threshold_margin, confidence) = if let Some(value) = baseline
    {
        let total_weight = eligible
            .iter()
            .map(|observation| weight(observation) as u128)
            .sum::<u128>();
        let weighted_uncertainty = if total_weight == 0 {
            0
        } else {
            eligible
                .iter()
                .map(|observation| {
                    u128::from(weight(observation)) * u128::from(observation.uncertainty_milli)
                })
                .sum::<u128>()
                .checked_div(total_weight)
                .unwrap_or(0) as u64
        };
        let weighted_deviation = if total_weight == 0 {
            0
        } else {
            eligible
                .iter()
                .map(|observation| {
                    u128::from(weight(observation))
                        * u128::from(abs_difference(observation.value_milli, value))
                })
                .sum::<u128>()
                .checked_div(total_weight)
                .unwrap_or(0) as u64
        };
        let half_width = weighted_uncertainty.saturating_add(weighted_deviation);
        let lower = clamp_value(i128::from(value) - i128::from(half_width));
        let upper = clamp_value(i128::from(value) + i128::from(half_width));
        let width = abs_difference(upper, lower);
        let margin = if supports(request.direction, lower, request.threshold_milli)
            && supports(request.direction, upper, request.threshold_milli)
        {
            abs_difference(value, request.threshold_milli)
        } else {
            0
        };
        let confidence = if request.max_interval_width_milli == 0 {
            0
        } else {
            (1_000u64.saturating_sub(
                width.saturating_mul(1_000) / (request.max_interval_width_milli + 1),
            )) as u16
        };
        (
            Some(lower),
            Some(upper),
            Some(width),
            Some(margin),
            confidence,
        )
    } else {
        (None, None, None, None, 0)
    };

    let missing_modalities = modality_order
        .iter()
        .copied()
        .filter(|modality| !by_modality.contains_key(modality))
        .collect::<Vec<_>>();
    let missing_required = required
        .iter()
        .any(|modality| !by_modality.contains_key(modality));
    let eligible_floor_failed = eligible.len() < request.min_modalities;
    let interval_too_wide =
        interval_width.is_some_and(|width| width > request.max_interval_width_milli);
    let confidence_too_low = confidence < request.min_confidence_milli;
    if eligible_floor_failed {
        negative.insert(format!(
            "eligible-modality-floor-not-met:{}",
            request.min_modalities
        ));
    }
    if interval_too_wide {
        uncertainty.insert("endpoint interval exceeds declared research gate".into());
    }
    if confidence_too_low && baseline.is_some() {
        uncertainty.insert("endpoint confidence is below declared research gate".into());
    }
    if !contradictory.is_empty() {
        uncertainty
            .insert("eligible modalities disagree with the aggregate threshold direction".into());
    }

    let disposition = if baseline.is_none() || eligible_floor_failed || missing_required {
        MultimodalDecisionGateDisposition::Blocked
    } else if interval_too_wide || confidence_too_low || !contradictory.is_empty() {
        MultimodalDecisionGateDisposition::Indeterminate
    } else if lower.is_some_and(|value| supports(request.direction, value, request.threshold_milli))
        && threshold_margin.is_some_and(|margin| margin >= request.min_margin_milli)
    {
        MultimodalDecisionGateDisposition::Supports
    } else if upper
        .is_some_and(|value| !supports(request.direction, value, request.threshold_milli))
        && threshold_margin == Some(0)
    {
        MultimodalDecisionGateDisposition::DoesNotSupport
    } else {
        MultimodalDecisionGateDisposition::Indeterminate
    };
    let next_action = match disposition {
        MultimodalDecisionGateDisposition::Supports => {
            "advance endpoint to mechanism or experiment planning"
        }
        MultimodalDecisionGateDisposition::DoesNotSupport => {
            "retain negative result and route to falsification or alternate mechanism"
        }
        MultimodalDecisionGateDisposition::Indeterminate => {
            "acquire orthogonal replication or improve measurement quality before advancing"
        }
        MultimodalDecisionGateDisposition::Blocked => {
            "acquire missing or eligible modality evidence before threshold interpretation"
        }
    }
    .into();
    let mut output = MultimodalDecisionGateAnalysis {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        objective: request.objective.clone(),
        endpoint_id: request.endpoint_id.clone(),
        study_id: request.study_id.clone(),
        model_system: request.model_system,
        direction: request.direction,
        threshold_milli: request.threshold_milli,
        modality_order,
        observations: modality_observations,
        baseline_value_milli: baseline,
        lower_bound_milli: lower,
        upper_bound_milli: upper,
        interval_width_milli: interval_width,
        threshold_margin_milli: threshold_margin,
        confidence_milli: confidence,
        eligible_modality_order: eligible_set.iter().copied().collect(),
        missing_modality_order: missing_modalities,
        contradictory_modality_order: contradictory.into_iter().collect(),
        negative_evidence: negative.into_iter().collect(),
        uncertainty: uncertainty.into_iter().collect(),
        disposition,
        next_action,
        digest: ContentHash::of_value(&serde_json::Value::Null)
            .map_err(|error| MultimodalDecisionGateError::Digest(error.to_string()))?,
    };
    output.digest = ContentHash::of_value(&digest_input(&output))
        .map_err(|error| MultimodalDecisionGateError::Digest(error.to_string()))?;
    validate_output(&output)?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn observation(
        modality: GliomaModality,
        value_milli: i64,
        uncertainty_milli: u64,
    ) -> EndpointEvidence {
        EndpointEvidence {
            modality,
            value_milli,
            uncertainty_milli,
            reliability_milli: 900,
            quality_milli: 900,
            replicate_count: 3,
        }
    }

    fn request(observations: Vec<EndpointEvidence>) -> MultimodalDecisionGateRequest {
        MultimodalDecisionGateRequest {
            objective: "gate invasion endpoint before autonomous mechanism planning".into(),
            endpoint_id: "invasion-score".into(),
            study_id: "decision-gate-study".into(),
            model_system: GliomaModelSystem::Organoid,
            required_modalities: vec![GliomaModality::Genomics, GliomaModality::Imaging],
            observations,
            direction: DecisionDirection::HigherSupports,
            threshold_milli: 600,
            min_modalities: 2,
            min_reliability_milli: 700,
            max_uncertainty_milli: 100,
            max_interval_width_milli: 250,
            min_margin_milli: 20,
            min_confidence_milli: 500,
        }
    }

    #[test]
    fn supported_endpoint_is_digest_bound() {
        let output = analyze_glioma_multimodal_decision_gate(&request(vec![
            observation(GliomaModality::Genomics, 800, 10),
            observation(GliomaModality::Imaging, 820, 10),
        ]))
        .expect("decision gate");
        assert_eq!(
            output.disposition,
            MultimodalDecisionGateDisposition::Supports
        );
        assert!(output.threshold_margin_milli.unwrap() >= 20);
        output.validate().expect("digest and invariants");
    }

    #[test]
    fn wide_interval_remains_indeterminate() {
        let mut request = request(vec![
            observation(GliomaModality::Genomics, 610, 100),
            observation(GliomaModality::Imaging, 590, 100),
        ]);
        request.max_interval_width_milli = 150;
        let output = analyze_glioma_multimodal_decision_gate(&request).expect("decision gate");
        assert_eq!(
            output.disposition,
            MultimodalDecisionGateDisposition::Indeterminate
        );
        assert!(output
            .uncertainty
            .iter()
            .any(|value| value.contains("interval")));
    }

    #[test]
    fn missing_required_modality_blocks_without_imputation() {
        let mut request = request(vec![observation(GliomaModality::Genomics, 800, 10)]);
        request.min_modalities = 1;
        let output = analyze_glioma_multimodal_decision_gate(&request).expect("blocked gate");
        assert_eq!(
            output.disposition,
            MultimodalDecisionGateDisposition::Blocked
        );
        assert!(output
            .missing_modality_order
            .contains(&GliomaModality::Imaging));
        assert!(output
            .negative_evidence
            .iter()
            .any(|value| value.contains("required-modality-missing")));
    }
}
