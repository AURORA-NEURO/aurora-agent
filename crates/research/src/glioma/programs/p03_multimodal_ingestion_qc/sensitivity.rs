//! Endpoint-sensitivity analysis for autonomous preclinical glioma research.
//!
//! Evidence fusion answers "what is the current aggregate estimate?".  This feature answers a
//! different operational question: "which measurement could change the scientific decision?" It
//! performs deterministic leave-one-modality-out and bounded perturbation analyses, so an
//! autonomous workflow can reacquire a fragile assay or proceed with a robust endpoint without
//! mistaking a single influential modality for a stable mechanism.  No value is imputed and the
//! routine never dispatches an assay or makes a clinical decision.

use super::evidence_fusion::EndpointEvidence;
use crate::glioma_engine::{GliomaModality, GliomaModelSystem};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeSet, HashMap};
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P03-F17";
pub const OUTPUT_SCHEMA: &str = "GliomaMultimodalSensitivity1@1";
pub const MAX_MODALITIES: usize = 64;
pub const MAX_ABS_VALUE_MILLI: u64 = 1_000_000_000;
pub const MAX_UNCERTAINTY_MILLI: u64 = 1_000_000_000;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SensitivityRequest {
    pub objective: String,
    pub endpoint_id: String,
    pub study_id: String,
    pub model_system: GliomaModelSystem,
    pub required_modalities: Vec<GliomaModality>,
    pub observations: Vec<EndpointEvidence>,
    pub min_modalities: usize,
    pub min_reliability_milli: u16,
    pub max_uncertainty_milli: u64,
    pub perturbation_milli: u64,
    pub max_perturbation_span_milli: u64,
    pub min_robustness_milli: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SensitivityDisposition {
    Ready,
    Conditional,
    Blocked,
    Unresolved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModalitySensitivity {
    pub modality: GliomaModality,
    pub eligible: bool,
    pub weight_milli: u64,
    pub baseline_contribution_milli: Option<i64>,
    pub leave_one_out_value_milli: Option<i64>,
    pub leave_one_out_delta_milli: Option<u64>,
    pub low_perturbation_value_milli: Option<i64>,
    pub high_perturbation_value_milli: Option<i64>,
    pub perturbation_span_milli: Option<u64>,
    pub robustness_milli: Option<u16>,
    pub sign_flip: bool,
    pub reason: String,
    pub recommended_action: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SensitivityAnalysis {
    pub feature_id: String,
    pub output_schema: String,
    pub objective: String,
    pub endpoint_id: String,
    pub study_id: String,
    pub model_system: GliomaModelSystem,
    pub modality_order: Vec<GliomaModality>,
    pub sensitivities: Vec<ModalitySensitivity>,
    pub baseline_value_milli: Option<i64>,
    pub eligible_modality_order: Vec<GliomaModality>,
    pub missing_modality_order: Vec<GliomaModality>,
    pub fragile_modality_order: Vec<GliomaModality>,
    pub acquisition_order: Vec<GliomaModality>,
    pub negative_evidence: Vec<String>,
    pub uncertainty: Vec<String>,
    pub disposition: SensitivityDisposition,
    pub next_action: String,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum SensitivityError {
    #[error("multimodal sensitivity request is invalid: {0}")]
    InvalidRequest(String),
    #[error("multimodal sensitivity input is invalid: {0}")]
    InvalidInput(String),
    #[error("multimodal sensitivity output is invalid: {0}")]
    InvalidOutput(String),
    #[error("multimodal sensitivity digest failed: {0}")]
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
    let denominator = u128::from(observation.uncertainty_milli) + 1;
    (numerator / denominator).min(u128::from(u64::MAX)) as u64
}

fn aggregate(observations: &[&EndpointEvidence]) -> Option<i64> {
    let mut numerator = 0i128;
    let mut denominator = 0u128;
    for observation in observations {
        let w = weight(observation);
        if w == 0 {
            continue;
        }
        numerator += i128::from(observation.value_milli) * i128::from(w);
        denominator += u128::from(w);
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

fn digest_input(output: &SensitivityAnalysis) -> serde_json::Value {
    serde_json::json!({
        "feature_id": output.feature_id,
        "output_schema": output.output_schema,
        "objective": output.objective,
        "endpoint_id": output.endpoint_id,
        "study_id": output.study_id,
        "model_system": output.model_system,
        "modality_order": output.modality_order,
        "sensitivities": output.sensitivities,
        "baseline_value_milli": output.baseline_value_milli,
        "eligible_modality_order": output.eligible_modality_order,
        "missing_modality_order": output.missing_modality_order,
        "fragile_modality_order": output.fragile_modality_order,
        "acquisition_order": output.acquisition_order,
        "negative_evidence": output.negative_evidence,
        "uncertainty": output.uncertainty,
        "disposition": output.disposition,
        "next_action": output.next_action,
    })
}

fn validate_request(request: &SensitivityRequest) -> Result<(), SensitivityError> {
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
        || request.perturbation_milli == 0
        || request.perturbation_milli > MAX_ABS_VALUE_MILLI
        || request.max_perturbation_span_milli == 0
        || request.max_perturbation_span_milli > MAX_ABS_VALUE_MILLI
        || request.min_robustness_milli > 1_000
    {
        return Err(SensitivityError::InvalidRequest(
            "endpoint/study identity, bounded observations, modality floor, and positive perturbation gates are required".into(),
        ));
    }
    if !canonical(&request.required_modalities) {
        return Err(SensitivityError::InvalidRequest(
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
            return Err(SensitivityError::InvalidInput(
                "observations require unique declared modalities, bounded values/uncertainty/quality/reliability, and positive replicate counts".into(),
            ));
        }
    }
    Ok(())
}

fn validate_output(output: &SensitivityAnalysis) -> Result<(), SensitivityError> {
    if output.feature_id != FEATURE_ID
        || output.output_schema != OUTPUT_SCHEMA
        || output.objective.trim().is_empty()
        || output.endpoint_id.trim().is_empty()
        || output.study_id.trim().is_empty()
        || !canonical(&output.modality_order)
        || !canonical(&output.eligible_modality_order)
        || !canonical(&output.missing_modality_order)
        || output.sensitivities.len() != output.modality_order.len()
        || output
            .sensitivities
            .windows(2)
            .any(|pair| pair[0].modality >= pair[1].modality)
        || !canonical(&output.negative_evidence)
        || !canonical(&output.uncertainty)
        || output
            .sensitivities
            .iter()
            .any(|row| row.reason.trim().is_empty() || row.recommended_action.trim().is_empty())
    {
        return Err(SensitivityError::InvalidOutput(
            "identity, modality coverage, sensitivity cardinality, or canonical evidence invariants are invalid".into(),
        ));
    }
    let mut seen_fragile = BTreeSet::new();
    if output
        .fragile_modality_order
        .iter()
        .any(|m| !seen_fragile.insert(*m))
    {
        return Err(SensitivityError::InvalidOutput(
            "fragile modalities must be unique".into(),
        ));
    }
    let mut seen_acquisition = BTreeSet::new();
    if output
        .acquisition_order
        .iter()
        .any(|m| !seen_acquisition.insert(*m))
    {
        return Err(SensitivityError::InvalidOutput(
            "acquisition order must be unique".into(),
        ));
    }
    let expected = ContentHash::of_value(&digest_input(output))
        .map_err(|error| SensitivityError::Digest(error.to_string()))?;
    if expected != output.digest {
        return Err(SensitivityError::InvalidOutput(
            "digest is not bound to multimodal sensitivity output".into(),
        ));
    }
    Ok(())
}

impl SensitivityAnalysis {
    pub fn validate(&self) -> Result<(), SensitivityError> {
        validate_output(self)
    }
}

/// Quantify endpoint fragility to modality removal and bounded measurement perturbations.
pub fn analyze_glioma_multimodal_sensitivity(
    request: &SensitivityRequest,
) -> Result<SensitivityAnalysis, SensitivityError> {
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
    let modality_order = required
        .union(&by_modality.keys().copied().collect::<BTreeSet<_>>())
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
    let baseline = aggregate(&eligible);
    let mut sensitivities = Vec::with_capacity(modality_order.len());
    let mut fragile_rank = Vec::new();
    let mut negative = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    let eligible_set = eligible.iter().map(|o| o.modality).collect::<BTreeSet<_>>();

    for modality in &modality_order {
        let Some(observation) = by_modality.get(modality) else {
            negative.insert(format!("required-modality-missing:{modality:?}"));
            sensitivities.push(ModalitySensitivity {
                modality: *modality,
                eligible: false,
                weight_milli: 0,
                baseline_contribution_milli: None,
                leave_one_out_value_milli: None,
                leave_one_out_delta_milli: None,
                low_perturbation_value_milli: None,
                high_perturbation_value_milli: None,
                perturbation_span_milli: None,
                robustness_milli: None,
                sign_flip: false,
                reason: "required observation is absent".into(),
                recommended_action: "acquire_missing_modality".into(),
            });
            continue;
        };
        let observation_weight = weight(observation);
        if !eligible_set.contains(modality) {
            negative.insert(format!("ineligible-modality:{modality:?}"));
            uncertainty.insert(format!("modality-gate-failed:{modality:?}"));
            sensitivities.push(ModalitySensitivity {
                modality: *modality,
                eligible: false,
                weight_milli: observation_weight,
                baseline_contribution_milli: None,
                leave_one_out_value_milli: None,
                leave_one_out_delta_milli: None,
                low_perturbation_value_milli: None,
                high_perturbation_value_milli: None,
                perturbation_span_milli: None,
                robustness_milli: None,
                sign_flip: false,
                reason: "reliability, quality, uncertainty, or weight gate failed".into(),
                recommended_action: "recalibrate_or_reacquire".into(),
            });
            continue;
        }
        let remaining = eligible
            .iter()
            .copied()
            .filter(|candidate| candidate.modality != *modality)
            .collect::<Vec<_>>();
        let leave_one_out = aggregate(&remaining);
        let leave_delta =
            leave_one_out.map(|value| abs_difference(value, baseline.unwrap_or(value)));
        let low_value = {
            let mut altered = eligible.iter().copied().collect::<Vec<_>>();
            let altered_observation = EndpointEvidence {
                value_milli: clamp_value(
                    i128::from(observation.value_milli) - i128::from(request.perturbation_milli),
                ),
                ..(*observation).clone()
            };
            altered.retain(|candidate| candidate.modality != *modality);
            altered.push(&altered_observation);
            aggregate(&altered)
        };
        let high_value = {
            let mut altered = eligible.iter().copied().collect::<Vec<_>>();
            let altered_observation = EndpointEvidence {
                value_milli: clamp_value(
                    i128::from(observation.value_milli) + i128::from(request.perturbation_milli),
                ),
                ..(*observation).clone()
            };
            altered.retain(|candidate| candidate.modality != *modality);
            altered.push(&altered_observation);
            aggregate(&altered)
        };
        let span = low_value
            .zip(high_value)
            .map(|(low, high)| abs_difference(high, low));
        let robustness = span.map(|value| {
            if value >= request.max_perturbation_span_milli {
                0
            } else {
                (1_000u64.saturating_sub(
                    value.saturating_mul(1_000) / (request.max_perturbation_span_milli + 1),
                )) as u16
            }
        });
        let sign_flip = baseline
            .zip(low_value)
            .is_some_and(|(base, low)| (base < 0 && low > 0) || (base > 0 && low < 0))
            || baseline
                .zip(high_value)
                .is_some_and(|(base, high)| (base < 0 && high > 0) || (base > 0 && high < 0));
        if sign_flip || robustness.is_some_and(|value| value < request.min_robustness_milli) {
            fragile_rank.push((*modality, span.unwrap_or(MAX_ABS_VALUE_MILLI)));
            uncertainty.insert(format!("endpoint-fragile-under:{modality:?}"));
        }
        let recommended_action = if sign_flip {
            "reacquire_before_mechanism_claim"
        } else if robustness.is_some_and(|value| value < request.min_robustness_milli) {
            "acquire_orthogonal_replication"
        } else if leave_delta.is_some_and(|value| value > request.max_perturbation_span_milli) {
            "retain_and_independently_replicate"
        } else {
            "retain_for_next_workflow_stage"
        };
        sensitivities.push(ModalitySensitivity {
            modality: *modality,
            eligible: true,
            weight_milli: observation_weight,
            baseline_contribution_milli: baseline
                .map(|value| value.saturating_sub(observation.value_milli)),
            leave_one_out_value_milli: leave_one_out,
            leave_one_out_delta_milli: leave_delta,
            low_perturbation_value_milli: low_value,
            high_perturbation_value_milli: high_value,
            perturbation_span_milli: span,
            robustness_milli: robustness,
            sign_flip,
            reason: "eligible measurement evaluated under removal and bounded perturbation".into(),
            recommended_action: recommended_action.into(),
        });
    }
    fragile_rank.sort_by(|left, right| right.1.cmp(&left.1).then_with(|| left.0.cmp(&right.0)));
    let fragile_modality_order = fragile_rank
        .iter()
        .map(|(modality, _)| *modality)
        .collect::<Vec<_>>();
    let missing_modality_order = modality_order
        .iter()
        .copied()
        .filter(|modality| !by_modality.contains_key(modality))
        .collect::<Vec<_>>();
    let mut acquisition_order = fragile_modality_order.clone();
    acquisition_order.extend(missing_modality_order.iter().copied());
    let additional_acquisition = modality_order
        .iter()
        .copied()
        .filter(|modality| {
            !acquisition_order.contains(modality) && !eligible_set.contains(modality)
        })
        .collect::<Vec<_>>();
    acquisition_order.extend(additional_acquisition);
    acquisition_order.sort_by(|left, right| {
        let left_rank = fragile_modality_order
            .iter()
            .position(|value| value == left)
            .unwrap_or(usize::MAX);
        let right_rank = fragile_modality_order
            .iter()
            .position(|value| value == right)
            .unwrap_or(usize::MAX);
        left_rank.cmp(&right_rank).then_with(|| left.cmp(right))
    });
    let missing_required = required
        .iter()
        .any(|modality| !by_modality.contains_key(modality));
    let eligible_floor_failed = eligible.len() < request.min_modalities;
    let sign_flip_present = sensitivities.iter().any(|row| row.sign_flip);
    let disposition = if baseline.is_none() || eligible_floor_failed || missing_required {
        SensitivityDisposition::Blocked
    } else if sign_flip_present {
        SensitivityDisposition::Unresolved
    } else if !fragile_modality_order.is_empty() || !negative.is_empty() {
        SensitivityDisposition::Conditional
    } else {
        SensitivityDisposition::Ready
    };
    if eligible_floor_failed {
        negative.insert(format!(
            "eligible-modality-floor-not-met:{}",
            request.min_modalities
        ));
    }
    if !fragile_modality_order.is_empty() {
        uncertainty
            .insert("endpoint estimate is sensitive to one or more modality perturbations".into());
    }
    let negative_evidence = negative.into_iter().collect::<Vec<_>>();
    let uncertainty = uncertainty.into_iter().collect::<Vec<_>>();
    let next_action = match disposition {
        SensitivityDisposition::Ready => "advance endpoint to mechanism or experiment planning",
        SensitivityDisposition::Conditional => {
            "reacquire or orthogonally replicate fragile modalities before advancing"
        }
        SensitivityDisposition::Blocked => {
            "acquire missing or eligible modality evidence before interpreting endpoint"
        }
        SensitivityDisposition::Unresolved => {
            "hold interpretation and adjudicate sign-changing modality evidence"
        }
    }
    .into();
    let mut output = SensitivityAnalysis {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        objective: request.objective.clone(),
        endpoint_id: request.endpoint_id.clone(),
        study_id: request.study_id.clone(),
        model_system: request.model_system,
        modality_order,
        sensitivities,
        baseline_value_milli: baseline,
        eligible_modality_order: eligible_set.iter().copied().collect(),
        missing_modality_order,
        fragile_modality_order,
        acquisition_order,
        negative_evidence,
        uncertainty,
        disposition,
        next_action,
        digest: ContentHash::of_value(&serde_json::Value::Null)
            .map_err(|error| SensitivityError::Digest(error.to_string()))?,
    };
    output.digest = ContentHash::of_value(&digest_input(&output))
        .map_err(|error| SensitivityError::Digest(error.to_string()))?;
    validate_output(&output)?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn observation(modality: GliomaModality, value_milli: i64) -> EndpointEvidence {
        EndpointEvidence {
            modality,
            value_milli,
            uncertainty_milli: 20,
            reliability_milli: 900,
            quality_milli: 900,
            replicate_count: 3,
        }
    }

    fn request(observations: Vec<EndpointEvidence>) -> SensitivityRequest {
        SensitivityRequest {
            objective: "rank evidence fragility before glioma mechanism planning".into(),
            endpoint_id: "invasion-score".into(),
            study_id: "sensitivity-study".into(),
            model_system: GliomaModelSystem::Organoid,
            required_modalities: vec![GliomaModality::Genomics, GliomaModality::Imaging],
            observations,
            min_modalities: 2,
            min_reliability_milli: 700,
            max_uncertainty_milli: 100,
            perturbation_milli: 100,
            max_perturbation_span_milli: 180,
            min_robustness_milli: 500,
        }
    }

    #[test]
    fn sensitivity_is_digest_bound_and_ranks_fragility() {
        let output = analyze_glioma_multimodal_sensitivity(&request(vec![
            observation(GliomaModality::Genomics, 700),
            observation(GliomaModality::Imaging, 500),
        ]))
        .expect("sensitivity analysis");
        assert_eq!(output.feature_id, FEATURE_ID);
        assert!(output.baseline_value_milli.is_some());
        assert_eq!(output.sensitivities.len(), 2);
        output.validate().expect("digest and invariants");
    }

    #[test]
    fn missing_required_modality_blocks_without_imputation() {
        let mut request = request(vec![observation(GliomaModality::Genomics, 700)]);
        request.min_modalities = 1;
        let output = analyze_glioma_multimodal_sensitivity(&request).expect("blocked output");
        assert_eq!(output.disposition, SensitivityDisposition::Blocked);
        assert_eq!(output.baseline_value_milli, Some(700));
        assert!(output
            .missing_modality_order
            .contains(&GliomaModality::Imaging));
        assert!(output
            .negative_evidence
            .iter()
            .any(|value| value.contains("required-modality-missing")));
    }

    #[test]
    fn duplicate_modalities_are_rejected() {
        let error = analyze_glioma_multimodal_sensitivity(&request(vec![
            observation(GliomaModality::Genomics, 700),
            observation(GliomaModality::Genomics, 710),
        ]))
        .expect_err("duplicate modality");
        assert!(matches!(error, SensitivityError::InvalidInput(_)));
    }
}
