//! Reliability- and uncertainty-aware endpoint evidence fusion for preclinical glioma research.
//!
//! This is the bridge between multimodal QC and downstream mechanism/experiment planning. It
//! combines only eligible modality estimates, computes a bounded precision-weighted endpoint
//! estimate, and refuses to collapse sign reversals, high dispersion, or missing required
//! modalities into a confident conclusion.

use crate::glioma_engine::{GliomaModality, GliomaModelSystem};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeSet, HashMap};
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P03-F09";
pub const OUTPUT_SCHEMA: &str = "GliomaMultimodalEvidenceFusion1@1";
pub const MAX_MODALITIES: usize = 64;
pub const MAX_ABS_VALUE_MILLI: u64 = 1_000_000_000;
pub const MAX_UNCERTAINTY_MILLI: u64 = 1_000_000_000;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EndpointEvidence {
    pub modality: GliomaModality,
    pub value_milli: i64,
    pub uncertainty_milli: u64,
    pub reliability_milli: u16,
    pub quality_milli: u16,
    pub replicate_count: u16,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceFusionRequest {
    pub objective: String,
    pub endpoint_id: String,
    pub study_id: String,
    pub model_system: GliomaModelSystem,
    pub required_modalities: Vec<GliomaModality>,
    pub observations: Vec<EndpointEvidence>,
    pub min_modalities: usize,
    pub min_reliability_milli: u16,
    pub max_uncertainty_milli: u64,
    pub max_contradiction_milli: u64,
    pub min_fused_confidence_milli: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceFusionDisposition {
    Ready,
    Conditional,
    Blocked,
    Unresolved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceContribution {
    pub modality: GliomaModality,
    pub eligible: bool,
    pub weight_milli: u64,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceFusionAnalysis {
    pub feature_id: String,
    pub output_schema: String,
    pub objective: String,
    pub endpoint_id: String,
    pub study_id: String,
    pub model_system: GliomaModelSystem,
    pub modality_order: Vec<GliomaModality>,
    pub contributions: Vec<EvidenceContribution>,
    pub fused_value_milli: Option<i64>,
    pub fused_uncertainty_milli: Option<u64>,
    pub dispersion_milli: Option<u64>,
    pub fused_confidence_milli: u16,
    pub eligible_modality_order: Vec<GliomaModality>,
    pub missing_modality_order: Vec<GliomaModality>,
    pub contradictory_modality_order: Vec<GliomaModality>,
    pub acquisition_order: Vec<GliomaModality>,
    pub negative_evidence: Vec<String>,
    pub uncertainty: Vec<String>,
    pub disposition: EvidenceFusionDisposition,
    pub next_action: String,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum EvidenceFusionError {
    #[error("evidence fusion request is invalid: {0}")]
    InvalidRequest(String),
    #[error("evidence fusion input is invalid: {0}")]
    InvalidInput(String),
    #[error("evidence fusion output is invalid: {0}")]
    InvalidOutput(String),
    #[error("evidence fusion digest failed: {0}")]
    Digest(String),
}

fn canonical<T: Ord>(values: &[T]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn digest_input(output: &EvidenceFusionAnalysis) -> serde_json::Value {
    serde_json::json!({
        "feature_id": output.feature_id,
        "output_schema": output.output_schema,
        "objective": output.objective,
        "endpoint_id": output.endpoint_id,
        "study_id": output.study_id,
        "model_system": output.model_system,
        "modality_order": output.modality_order,
        "contributions": output.contributions,
        "fused_value_milli": output.fused_value_milli,
        "fused_uncertainty_milli": output.fused_uncertainty_milli,
        "dispersion_milli": output.dispersion_milli,
        "fused_confidence_milli": output.fused_confidence_milli,
        "eligible_modality_order": output.eligible_modality_order,
        "missing_modality_order": output.missing_modality_order,
        "contradictory_modality_order": output.contradictory_modality_order,
        "acquisition_order": output.acquisition_order,
        "negative_evidence": output.negative_evidence,
        "uncertainty": output.uncertainty,
        "disposition": output.disposition,
        "next_action": output.next_action,
    })
}

fn validate_request(request: &EvidenceFusionRequest) -> Result<(), EvidenceFusionError> {
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
        || request.max_contradiction_milli > MAX_ABS_VALUE_MILLI
        || request.min_fused_confidence_milli > 1_000
    {
        return Err(EvidenceFusionError::InvalidRequest(
            "endpoint/study identity, bounded unique observations, modality floor, and uncertainty/confidence gates are required".into(),
        ));
    }
    if !canonical(&request.required_modalities) {
        return Err(EvidenceFusionError::InvalidRequest(
            "required modalities must be unique and canonical".into(),
        ));
    }
    let required = request
        .required_modalities
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    let mut modalities = BTreeSet::new();
    for observation in &request.observations {
        if !modalities.insert(observation.modality)
            || (!required.is_empty() && !required.contains(&observation.modality))
            || observation.value_milli.unsigned_abs() > MAX_ABS_VALUE_MILLI
            || observation.uncertainty_milli > MAX_UNCERTAINTY_MILLI
            || observation.reliability_milli > 1_000
            || observation.quality_milli > 1_000
            || observation.replicate_count == 0
        {
            return Err(EvidenceFusionError::InvalidInput(
                "observations require unique declared modalities, bounded values/uncertainty/quality/reliability, and positive replicate counts".into(),
            ));
        }
    }
    Ok(())
}

fn validate_output(output: &EvidenceFusionAnalysis) -> Result<(), EvidenceFusionError> {
    if output.feature_id != FEATURE_ID
        || output.output_schema != OUTPUT_SCHEMA
        || output.objective.trim().is_empty()
        || output.endpoint_id.trim().is_empty()
        || output.study_id.trim().is_empty()
        || !canonical(&output.modality_order)
        || !canonical(&output.eligible_modality_order)
        || !canonical(&output.missing_modality_order)
        || !canonical(&output.contradictory_modality_order)
        || !canonical(&output.acquisition_order)
        || !canonical(&output.negative_evidence)
        || !canonical(&output.uncertainty)
        || output.fused_confidence_milli > 1_000
        || output.contributions.len() != output.modality_order.len()
        || output
            .contributions
            .windows(2)
            .any(|pair| pair[0].modality >= pair[1].modality)
        || output.contributions.iter().any(|contribution| {
            contribution.weight_milli == 0 && contribution.eligible
                || contribution.reason.trim().is_empty()
        })
    {
        return Err(EvidenceFusionError::InvalidOutput(
            "identity, ordering, contribution cardinality, bounded confidence, or reason invariants are invalid".into(),
        ));
    }
    let expected = ContentHash::of_value(&digest_input(output))
        .map_err(|error| EvidenceFusionError::Digest(error.to_string()))?;
    if expected != output.digest {
        return Err(EvidenceFusionError::InvalidOutput(
            "digest is not bound to evidence-fusion output".into(),
        ));
    }
    Ok(())
}

impl EvidenceFusionAnalysis {
    pub fn validate(&self) -> Result<(), EvidenceFusionError> {
        validate_output(self)
    }
}

/// Fuse eligible local preclinical modality estimates for one declared endpoint.
pub fn analyze_glioma_multimodal_evidence_fusion(
    request: &EvidenceFusionRequest,
) -> Result<EvidenceFusionAnalysis, EvidenceFusionError> {
    validate_request(request)?;
    let modality_order = request
        .required_modalities
        .iter()
        .copied()
        .chain(
            request
                .observations
                .iter()
                .map(|observation| observation.modality),
        )
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let by_modality = request
        .observations
        .iter()
        .map(|observation| (observation.modality, observation))
        .collect::<HashMap<_, _>>();
    let mut contributions = Vec::new();
    let mut eligible = Vec::new();
    let mut missing = Vec::new();
    let mut acquisition = BTreeSet::new();
    for modality in &modality_order {
        let Some(observation) = by_modality.get(modality).copied() else {
            missing.push(*modality);
            acquisition.insert(*modality);
            contributions.push(EvidenceContribution {
                modality: *modality,
                eligible: false,
                weight_milli: 0,
                reason: "modality was required but no endpoint observation was supplied".into(),
            });
            continue;
        };
        let eligible_observation = observation.reliability_milli >= request.min_reliability_milli
            && observation.quality_milli >= request.min_reliability_milli
            && observation.uncertainty_milli <= request.max_uncertainty_milli;
        if eligible_observation {
            let weight = u64::from(observation.reliability_milli)
                .saturating_mul(u64::from(observation.quality_milli))
                .saturating_mul(u64::from(observation.replicate_count))
                .saturating_mul(1_000)
                / observation.uncertainty_milli.saturating_add(1);
            eligible.push(*modality);
            contributions.push(EvidenceContribution {
                modality: *modality,
                eligible: true,
                weight_milli: weight.max(1),
                reason: "eligible at declared reliability, quality, and uncertainty gates".into(),
            });
        } else {
            missing.push(*modality);
            acquisition.insert(*modality);
            let reason = if observation.uncertainty_milli > request.max_uncertainty_milli {
                "uncertainty exceeds endpoint gate"
            } else if observation.reliability_milli < request.min_reliability_milli {
                "reliability below endpoint gate"
            } else {
                "quality below endpoint gate"
            };
            contributions.push(EvidenceContribution {
                modality: *modality,
                eligible: false,
                weight_milli: 0,
                reason: reason.into(),
            });
        }
    }
    let mut negative_evidence = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    let (fused_value_milli, fused_uncertainty_milli, dispersion_milli, base_confidence) =
        if eligible.is_empty() {
            (None, None, None, 0_u16)
        } else {
            let denominator = eligible
                .iter()
                .map(|modality| {
                    contributions
                        .iter()
                        .find(|c| c.modality == *modality)
                        .unwrap()
                        .weight_milli
                })
                .sum::<u64>()
                .max(1);
            let numerator = eligible
                .iter()
                .map(|modality| {
                    let observation = by_modality[modality];
                    let weight = contributions
                        .iter()
                        .find(|contribution| contribution.modality == *modality)
                        .unwrap()
                        .weight_milli;
                    i128::from(observation.value_milli) * i128::from(weight)
                })
                .sum::<i128>();
            let fused = (numerator / i128::from(denominator))
                .clamp(i128::from(i64::MIN), i128::from(i64::MAX)) as i64;
            let dispersion = eligible
                .iter()
                .map(|modality| {
                    let observation = by_modality[modality];
                    let weight = contributions
                        .iter()
                        .find(|contribution| contribution.modality == *modality)
                        .unwrap()
                        .weight_milli;
                    weight.saturating_mul(
                        observation.value_milli.saturating_sub(fused).unsigned_abs(),
                    )
                })
                .sum::<u64>()
                / denominator;
            let uncertainty = eligible
                .iter()
                .map(|modality| {
                    let observation = by_modality[modality];
                    let weight = contributions
                        .iter()
                        .find(|contribution| contribution.modality == *modality)
                        .unwrap()
                        .weight_milli;
                    weight.saturating_mul(observation.uncertainty_milli)
                })
                .sum::<u64>()
                / denominator;
            let weighted_quality = eligible
                .iter()
                .map(|modality| {
                    let observation = by_modality[modality];
                    let weight = contributions
                        .iter()
                        .find(|contribution| contribution.modality == *modality)
                        .unwrap()
                        .weight_milli;
                    weight.saturating_mul(u64::from(
                        observation.reliability_milli.min(observation.quality_milli),
                    ))
                })
                .sum::<u64>()
                / denominator;
            (
                Some(fused),
                Some(uncertainty.saturating_add(dispersion)),
                Some(dispersion),
                weighted_quality.min(1_000) as u16,
            )
        };
    let mut contradictory = BTreeSet::new();
    if eligible.len() >= 2 {
        for left_index in 0..eligible.len() {
            for right_index in left_index + 1..eligible.len() {
                let left = by_modality[&eligible[left_index]].value_milli;
                let right = by_modality[&eligible[right_index]].value_milli;
                if (left != 0 && right != 0 && left.signum() != right.signum())
                    || left.saturating_sub(right).unsigned_abs() > request.max_contradiction_milli
                {
                    contradictory.insert(eligible[left_index]);
                    contradictory.insert(eligible[right_index]);
                }
            }
        }
    }
    if !contradictory.is_empty() {
        negative_evidence.insert("eligible-modalities-contradict-endpoint".into());
    }
    if !missing.is_empty() {
        uncertainty.insert("one-or-more-modalities-failed-eligibility-gates".into());
    }
    let uncertainty_penalty = fused_uncertainty_milli
        .map(|value| {
            ((value.min(request.max_uncertainty_milli.saturating_mul(2))).saturating_mul(250)
                / request.max_uncertainty_milli.max(1))
            .min(250)
        })
        .unwrap_or(500);
    let dispersion_penalty = dispersion_milli
        .map(|value| {
            ((value.min(request.max_contradiction_milli.saturating_mul(2))).saturating_mul(250)
                / request.max_contradiction_milli.max(1))
            .min(250)
        })
        .unwrap_or(500);
    let fused_confidence_milli = base_confidence
        .saturating_sub(uncertainty_penalty as u16)
        .saturating_sub(dispersion_penalty as u16);
    if eligible.is_empty() {
        negative_evidence.insert("no-modality-passed-fusion-gates".into());
    }
    let disposition = if eligible.is_empty() {
        EvidenceFusionDisposition::Unresolved
    } else if !contradictory.is_empty() {
        EvidenceFusionDisposition::Blocked
    } else if eligible.len() < request.min_modalities
        || !missing.is_empty()
        || fused_confidence_milli < request.min_fused_confidence_milli
    {
        EvidenceFusionDisposition::Conditional
    } else {
        EvidenceFusionDisposition::Ready
    };
    let next_action = match disposition {
        EvidenceFusionDisposition::Ready => {
            "fused endpoint evidence is eligible for downstream mechanism or experiment planning".into()
        }
        EvidenceFusionDisposition::Conditional => {
            "acquire or improve missing modalities and raise fused confidence before downstream planning".into()
        }
        EvidenceFusionDisposition::Blocked => {
            "reconcile contradictory modality estimates before any downstream interpretation".into()
        }
        EvidenceFusionDisposition::Unresolved => {
            "collect quality-controlled endpoint evidence before attempting fusion".into()
        }
    };
    let mut output = EvidenceFusionAnalysis {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        objective: request.objective.clone(),
        endpoint_id: request.endpoint_id.clone(),
        study_id: request.study_id.clone(),
        model_system: request.model_system,
        modality_order,
        contributions,
        fused_value_milli,
        fused_uncertainty_milli,
        dispersion_milli,
        fused_confidence_milli,
        eligible_modality_order: eligible,
        missing_modality_order: missing,
        contradictory_modality_order: contradictory.into_iter().collect(),
        acquisition_order: acquisition.into_iter().collect(),
        negative_evidence: negative_evidence.into_iter().collect(),
        uncertainty: uncertainty.into_iter().collect(),
        disposition,
        next_action,
        digest: ContentHash::of_bytes(b"unsealed-glioma-multimodal-evidence-fusion"),
    };
    output.digest = ContentHash::of_value(&digest_input(&output))
        .map_err(|error| EvidenceFusionError::Digest(error.to_string()))?;
    validate_output(&output)?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request(observations: Vec<EndpointEvidence>) -> EvidenceFusionRequest {
        EvidenceFusionRequest {
            objective: "fuse multimodal invasion endpoint evidence".into(),
            endpoint_id: "invasion".into(),
            study_id: "study-1".into(),
            model_system: GliomaModelSystem::Organoid,
            required_modalities: vec![GliomaModality::Genomics, GliomaModality::Imaging],
            observations,
            min_modalities: 2,
            min_reliability_milli: 700,
            max_uncertainty_milli: 100,
            max_contradiction_milli: 300,
            min_fused_confidence_milli: 400,
        }
    }

    #[test]
    fn evidence_fusion_releases_consistent_endpoint() {
        let output = analyze_glioma_multimodal_evidence_fusion(&request(vec![
            EndpointEvidence {
                modality: GliomaModality::Genomics,
                value_milli: 500,
                uncertainty_milli: 20,
                reliability_milli: 900,
                quality_milli: 900,
                replicate_count: 3,
            },
            EndpointEvidence {
                modality: GliomaModality::Imaging,
                value_milli: 520,
                uncertainty_milli: 30,
                reliability_milli: 850,
                quality_milli: 900,
                replicate_count: 3,
            },
        ]))
        .unwrap();
        assert_eq!(output.disposition, EvidenceFusionDisposition::Ready);
        assert!(output.fused_value_milli.unwrap() >= 500);
        output.validate().unwrap();
    }

    #[test]
    fn evidence_fusion_blocks_sign_reversal() {
        let output = analyze_glioma_multimodal_evidence_fusion(&request(vec![
            EndpointEvidence {
                modality: GliomaModality::Genomics,
                value_milli: 500,
                uncertainty_milli: 20,
                reliability_milli: 900,
                quality_milli: 900,
                replicate_count: 3,
            },
            EndpointEvidence {
                modality: GliomaModality::Imaging,
                value_milli: -500,
                uncertainty_milli: 20,
                reliability_milli: 900,
                quality_milli: 900,
                replicate_count: 3,
            },
        ]))
        .unwrap();
        assert_eq!(output.disposition, EvidenceFusionDisposition::Blocked);
        assert_eq!(
            output.contradictory_modality_order,
            vec![GliomaModality::Genomics, GliomaModality::Imaging]
        );
        assert!(output
            .negative_evidence
            .iter()
            .any(|item| item.contains("contradict")));
    }
}
