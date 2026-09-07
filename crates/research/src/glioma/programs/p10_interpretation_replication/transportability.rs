//! Transportability analysis across preclinical glioma model systems.
//!
//! A result observed in one model system is not automatically portable to another. This module
//! estimates a deterministic similarity- and quality-weighted effect for a declared target model,
//! keeps model disagreement and covariate distance visible, and computes leave-one-study-out
//! stability. It is a research interpretation product, never a clinical or treatment decision.

use crate::glioma_engine::{GliomaModelSystem, LocalArtifactRef};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P10-F17";
pub const OUTPUT_SCHEMA: &str = "GliomaTransportabilityAnalysis1@1";
pub const MAX_STUDIES: usize = 4_096;
pub const MAX_SIGNATURE_DIMENSIONS: usize = 256;
const WEIGHT_SCALE: u128 = 1_000_000_000;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TransportabilityRequest {
    pub objective: String,
    pub target_model_system: GliomaModelSystem,
    pub target_signature: Vec<i64>,
    pub min_studies: usize,
    pub min_replicates_per_study: u16,
    pub min_quality_milli: u16,
    pub distance_scale_milli: u32,
    pub max_transport_gap_milli: u16,
    pub max_heterogeneity_milli: u16,
    pub effect_threshold_milli: u64,
    pub min_signal_to_noise_milli: u64,
    pub max_leave_one_out_shift_milli: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TransportStudy {
    pub study_id: String,
    pub model_system: GliomaModelSystem,
    pub population_signature: Vec<i64>,
    pub effect_milli: i64,
    pub uncertainty_milli: u64,
    pub replicates: u16,
    pub quality_milli: u16,
    pub artifact: LocalArtifactRef,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TransportStudyContribution {
    pub study_id: String,
    pub source_model_system: GliomaModelSystem,
    pub distance_milli: u32,
    pub similarity_milli: u32,
    pub weight_milli: u64,
    pub effect_milli: i64,
    pub leave_one_out_shift_milli: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TransportabilityDisposition {
    Qualified,
    Partial,
    Heterogeneous,
    Negative,
    Unresolved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TransportabilityAnalysis {
    pub feature_id: String,
    pub output_schema: String,
    pub objective: String,
    pub target_model_system: GliomaModelSystem,
    pub study_order: Vec<String>,
    pub included_order: Vec<String>,
    pub excluded_order: Vec<String>,
    pub contributions: Vec<TransportStudyContribution>,
    pub pooled_effect_milli: i64,
    pub pooled_uncertainty_milli: u64,
    pub signal_to_noise_milli: u64,
    pub transport_gap_milli: u16,
    pub heterogeneity_milli: u16,
    pub max_leave_one_out_shift_milli: u64,
    pub negative_evidence: Vec<String>,
    pub uncertainty: Vec<String>,
    pub disposition: TransportabilityDisposition,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum TransportabilityError {
    #[error("transportability request is invalid: {0}")]
    InvalidRequest(String),
    #[error("transportability study is invalid: {0}")]
    InvalidStudy(String),
    #[error("transportability output is invalid: {0}")]
    InvalidOutput(String),
    #[error("transportability digest failed: {0}")]
    Digest(String),
}

fn canonical(values: &[String]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn distance(left: &[i64], right: &[i64]) -> u64 {
    left.iter()
        .zip(right)
        .map(|(a, b)| a.saturating_sub(*b).unsigned_abs())
        .fold(0_u64, |sum, value| sum.saturating_add(value))
        .min(u64::from(u32::MAX))
}

fn similarity(distance: u64, scale_milli: u32) -> u32 {
    let scaled = u128::from(distance)
        .saturating_mul(1_000)
        .checked_div(u128::from(scale_milli.max(1)))
        .unwrap_or(u128::from(u32::MAX));
    1_000_u32.saturating_sub(scaled.min(1_000) as u32)
}

fn weighted_effect(studies: &[&TransportStudy], request: &TransportabilityRequest) -> (i64, u128) {
    let mut total = 0_u128;
    let mut numerator = 0_i128;
    for study in studies {
        let d = distance(&study.population_signature, &request.target_signature);
        let sim = u128::from(similarity(d, request.distance_scale_milli));
        let quality = u128::from(study.quality_milli);
        let replicates = u128::from(study.replicates.max(1));
        let uncertainty = u128::from(study.uncertainty_milli.max(1));
        let weight = sim
            .saturating_mul(quality)
            .saturating_mul(replicates)
            .saturating_mul(WEIGHT_SCALE)
            .checked_div(uncertainty.saturating_mul(uncertainty).max(1))
            .unwrap_or(0);
        total = total.saturating_add(weight);
        numerator =
            numerator.saturating_add(i128::from(study.effect_milli).saturating_mul(weight as i128));
    }
    if total == 0 {
        (0, 0)
    } else {
        (
            (numerator / total as i128).clamp(i128::from(i64::MIN), i128::from(i64::MAX)) as i64,
            total,
        )
    }
}

fn digest_input(output: &TransportabilityAnalysis) -> serde_json::Value {
    serde_json::json!({
        "feature_id": output.feature_id,
        "output_schema": output.output_schema,
        "objective": output.objective,
        "target_model_system": output.target_model_system,
        "study_order": output.study_order,
        "included_order": output.included_order,
        "excluded_order": output.excluded_order,
        "contributions": output.contributions,
        "pooled_effect_milli": output.pooled_effect_milli,
        "pooled_uncertainty_milli": output.pooled_uncertainty_milli,
        "signal_to_noise_milli": output.signal_to_noise_milli,
        "transport_gap_milli": output.transport_gap_milli,
        "heterogeneity_milli": output.heterogeneity_milli,
        "max_leave_one_out_shift_milli": output.max_leave_one_out_shift_milli,
        "negative_evidence": output.negative_evidence,
        "uncertainty": output.uncertainty,
        "disposition": output.disposition,
    })
}

impl TransportabilityAnalysis {
    pub fn validate(&self) -> Result<(), TransportabilityError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || self.objective.trim().is_empty()
            || !canonical(&self.study_order)
            || !canonical(&self.included_order)
            || !canonical(&self.excluded_order)
            || !canonical(&self.negative_evidence)
            || !canonical(&self.uncertainty)
            || self.transport_gap_milli > 1_000
            || self.heterogeneity_milli > 1_000
            || self.contributions.len() != self.included_order.len()
            || self.contributions.iter().any(|contribution| {
                contribution.study_id.trim().is_empty()
                    || contribution.similarity_milli > 1_000
                    || contribution.distance_milli > 1_000_000_000
            })
        {
            return Err(TransportabilityError::InvalidOutput(
                "identity, ordering, bounds, or contribution cardinality is invalid".into(),
            ));
        }
        let included = self.included_order.iter().cloned().collect::<BTreeSet<_>>();
        let contribution_ids = self
            .contributions
            .iter()
            .map(|contribution| contribution.study_id.clone())
            .collect::<BTreeSet<_>>();
        if included != contribution_ids
            || self
                .included_order
                .iter()
                .any(|id| self.excluded_order.contains(id))
        {
            return Err(TransportabilityError::InvalidOutput(
                "included/excluded/contribution identities do not reconcile".into(),
            ));
        }
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| TransportabilityError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(TransportabilityError::InvalidOutput(
                "transportability digest is not content-addressed".into(),
            ));
        }
        Ok(())
    }
}

fn validate_request(request: &TransportabilityRequest) -> Result<(), TransportabilityError> {
    if request.objective.trim().is_empty()
        || request.target_signature.is_empty()
        || request.target_signature.len() > MAX_SIGNATURE_DIMENSIONS
        || request.min_studies == 0
        || request.min_replicates_per_study == 0
        || request.min_quality_milli > 1_000
        || request.distance_scale_milli == 0
        || request.max_transport_gap_milli > 1_000
        || request.max_heterogeneity_milli > 1_000
        || request.effect_threshold_milli == 0
        || request.min_signal_to_noise_milli == 0
    {
        return Err(TransportabilityError::InvalidRequest(
            "objective, target signature, study floors, distance scale, and bounded gates are required".into(),
        ));
    }
    Ok(())
}

fn validate_study(
    study: &TransportStudy,
    request: &TransportabilityRequest,
) -> Result<(), TransportabilityError> {
    if study.study_id.trim().is_empty()
        || study.population_signature.len() != request.target_signature.len()
        || study.population_signature.len() > MAX_SIGNATURE_DIMENSIONS
        || study.uncertainty_milli == 0
        || study.quality_milli > 1_000
        || study.replicates == 0
    {
        return Err(TransportabilityError::InvalidStudy(
            "study identity, signature dimension, uncertainty, quality, and replicate bounds are required".into(),
        ));
    }
    study
        .artifact
        .validate()
        .map_err(|error| TransportabilityError::InvalidStudy(error.to_string()))
}

/// Estimate whether a preclinical glioma effect transports to a declared target model system.
pub fn analyze_glioma_transportability(
    request: &TransportabilityRequest,
    studies: &[TransportStudy],
) -> Result<TransportabilityAnalysis, TransportabilityError> {
    validate_request(request)?;
    if studies.is_empty() || studies.len() > MAX_STUDIES {
        return Err(TransportabilityError::InvalidStudy(
            "a bounded non-empty study set is required".into(),
        ));
    }
    let mut study_ids = BTreeSet::new();
    let mut study_order = studies
        .iter()
        .map(|study| study.study_id.clone())
        .collect::<Vec<_>>();
    study_order.sort();
    if studies.iter().any(|study| {
        validate_study(study, request).is_err() || !study_ids.insert(study.study_id.clone())
    }) {
        return Err(TransportabilityError::InvalidStudy(
            "study ids must be unique and every study must satisfy the typed local contract".into(),
        ));
    }
    let mut included = Vec::new();
    let mut excluded = Vec::new();
    let mut uncertainty = BTreeSet::new();
    let mut negative = BTreeSet::new();
    for study in studies {
        if study.replicates < request.min_replicates_per_study {
            excluded.push(study.study_id.clone());
            uncertainty.insert(format!("{}:replicate-floor", study.study_id));
        } else if study.quality_milli < request.min_quality_milli {
            excluded.push(study.study_id.clone());
            uncertainty.insert(format!("{}:quality-floor", study.study_id));
        } else {
            included.push(study);
        }
    }
    included.sort_by(|left, right| left.study_id.cmp(&right.study_id));
    excluded.sort();
    let (pooled_effect, total_weight) = weighted_effect(&included, request);
    let mut contributions = Vec::new();
    for study in &included {
        let d = distance(&study.population_signature, &request.target_signature);
        let sim = similarity(d, request.distance_scale_milli);
        let shift = if included.len() <= 1 {
            0
        } else {
            let without = included
                .iter()
                .copied()
                .filter(|candidate| candidate.study_id != study.study_id)
                .collect::<Vec<_>>();
            weighted_effect(&without, request)
                .0
                .saturating_sub(pooled_effect)
                .unsigned_abs()
        };
        let uncertainty_sq = u128::from(study.uncertainty_milli)
            .saturating_mul(u128::from(study.uncertainty_milli))
            .max(1);
        let raw_weight = u128::from(sim)
            .saturating_mul(u128::from(study.quality_milli))
            .saturating_mul(u128::from(study.replicates))
            .saturating_mul(WEIGHT_SCALE)
            .checked_div(uncertainty_sq)
            .unwrap_or(0);
        contributions.push(TransportStudyContribution {
            study_id: study.study_id.clone(),
            source_model_system: study.model_system,
            distance_milli: d.min(u64::from(u32::MAX)) as u32,
            similarity_milli: sim,
            weight_milli: raw_weight.min(u64::MAX as u128) as u64,
            effect_milli: study.effect_milli,
            leave_one_out_shift_milli: shift,
        });
    }
    let absolute_deviation = included
        .iter()
        .map(|study| {
            let d = distance(&study.population_signature, &request.target_signature);
            let sim = u128::from(similarity(d, request.distance_scale_milli));
            let weight = sim
                .saturating_mul(u128::from(study.quality_milli))
                .saturating_mul(u128::from(study.replicates))
                .saturating_mul(WEIGHT_SCALE)
                .checked_div(
                    u128::from(study.uncertainty_milli)
                        .saturating_mul(u128::from(study.uncertainty_milli))
                        .max(1),
                )
                .unwrap_or(0);
            (study
                .effect_milli
                .saturating_sub(pooled_effect)
                .unsigned_abs() as u128)
                .saturating_mul(weight)
        })
        .sum::<u128>();
    let heterogeneity = absolute_deviation
        .checked_div(total_weight)
        .unwrap_or(1_000)
        .min(1_000) as u16;
    let transport_gap = if total_weight == 0 {
        1_000
    } else {
        let (weighted_distance, distance_weight) =
            included
                .iter()
                .fold((0_u128, 0_u128), |(distance_sum, weight_sum), study| {
                    let d = u128::from(distance(
                        &study.population_signature,
                        &request.target_signature,
                    ));
                    let sim = u128::from(similarity(d as u64, request.distance_scale_milli));
                    let weight = sim
                        .saturating_mul(u128::from(study.quality_milli))
                        .saturating_mul(u128::from(study.replicates));
                    (
                        distance_sum.saturating_add(d.saturating_mul(weight)),
                        weight_sum.saturating_add(weight),
                    )
                });
        (weighted_distance
            .checked_div(distance_weight.max(1))
            .unwrap_or(0)
            .saturating_mul(1_000)
            .checked_div(u128::from(request.distance_scale_milli).max(1))
            .unwrap_or(1_000)
            .min(1_000)) as u16
    };
    let pooled_uncertainty = included
        .iter()
        .map(|study| study.uncertainty_milli)
        .max()
        .unwrap_or(0)
        .saturating_add(u64::from(heterogeneity));
    let signal_to_noise =
        pooled_effect.unsigned_abs().saturating_mul(1_000) / pooled_uncertainty.max(1);
    let max_shift = contributions
        .iter()
        .map(|contribution| contribution.leave_one_out_shift_milli)
        .max()
        .unwrap_or(0);
    if pooled_effect.unsigned_abs() < request.effect_threshold_milli
        || signal_to_noise < request.min_signal_to_noise_milli
    {
        negative.insert("transported-effect-does-not-clear-signal-threshold".into());
    }
    if transport_gap > request.max_transport_gap_milli {
        uncertainty.insert("target-model-is-distant-from-source-signatures".into());
    }
    if heterogeneity > request.max_heterogeneity_milli {
        uncertainty.insert("source-effects-are-heterogeneous".into());
    }
    if max_shift > request.max_leave_one_out_shift_milli {
        uncertainty.insert("transported-effect-is-leave-one-study-sensitive".into());
    }
    let disposition = if included.len() < request.min_studies {
        TransportabilityDisposition::Unresolved
    } else if total_weight == 0 {
        TransportabilityDisposition::Partial
    } else if heterogeneity > request.max_heterogeneity_milli {
        TransportabilityDisposition::Heterogeneous
    } else if !negative.is_empty() {
        TransportabilityDisposition::Negative
    } else if !uncertainty.is_empty() {
        TransportabilityDisposition::Partial
    } else {
        TransportabilityDisposition::Qualified
    };
    let mut output = TransportabilityAnalysis {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        objective: request.objective.clone(),
        target_model_system: request.target_model_system,
        study_order,
        included_order: included
            .iter()
            .map(|study| study.study_id.clone())
            .collect(),
        excluded_order: excluded,
        contributions,
        pooled_effect_milli: pooled_effect,
        pooled_uncertainty_milli: pooled_uncertainty,
        signal_to_noise_milli: signal_to_noise,
        transport_gap_milli: transport_gap,
        heterogeneity_milli: heterogeneity,
        max_leave_one_out_shift_milli: max_shift,
        negative_evidence: negative.into_iter().collect(),
        uncertainty: uncertainty.into_iter().collect(),
        disposition,
        digest: ContentHash::of_bytes(b"unsealed-glioma-transportability"),
    };
    output.digest = ContentHash::of_value(&digest_input(&output))
        .map_err(|error| TransportabilityError::Digest(error.to_string()))?;
    output.validate()?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

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

    fn request() -> TransportabilityRequest {
        TransportabilityRequest {
            objective: "transport an invasion effect to organoids".into(),
            target_model_system: GliomaModelSystem::Organoid,
            target_signature: vec![100, 100],
            min_studies: 2,
            min_replicates_per_study: 3,
            min_quality_milli: 700,
            distance_scale_milli: 200,
            max_transport_gap_milli: 700,
            max_heterogeneity_milli: 250,
            effect_threshold_milli: 100,
            min_signal_to_noise_milli: 500,
            max_leave_one_out_shift_milli: 500,
        }
    }

    fn study(id: &str, signature: Vec<i64>, effect: i64) -> TransportStudy {
        TransportStudy {
            study_id: id.into(),
            model_system: GliomaModelSystem::Organoid,
            population_signature: signature,
            effect_milli: effect,
            uncertainty_milli: 40,
            replicates: 4,
            quality_milli: 900,
            artifact: artifact(id),
        }
    }

    #[test]
    fn similar_preclinical_studies_transport_and_replay() {
        let studies = vec![
            study("a", vec![100, 100], 400),
            study("b", vec![110, 95], 420),
        ];
        let first = analyze_glioma_transportability(&request(), &studies).unwrap();
        let mut reversed = studies;
        reversed.reverse();
        let second = analyze_glioma_transportability(&request(), &reversed).unwrap();
        assert_eq!(first, second);
        assert_eq!(first.disposition, TransportabilityDisposition::Qualified);
        first.validate().unwrap();
    }

    #[test]
    fn distant_source_is_partial_not_silently_portable() {
        let mut studies = vec![
            study("a", vec![900, 900], 400),
            study("b", vec![900, 900], 420),
        ];
        let mut req = request();
        req.max_transport_gap_milli = 100;
        let output = analyze_glioma_transportability(&req, &studies).unwrap();
        assert!(output
            .uncertainty
            .iter()
            .any(|item| item.contains("distant")));
        assert_eq!(output.disposition, TransportabilityDisposition::Partial);
        studies[0].effect_milli = -400;
        let heterogeneous = analyze_glioma_transportability(&req, &studies).unwrap();
        assert!(matches!(
            heterogeneous.disposition,
            TransportabilityDisposition::Heterogeneous
                | TransportabilityDisposition::Negative
                | TransportabilityDisposition::Partial
        ));
    }

    #[test]
    fn replicate_floor_is_unresolved_and_negative_effect_is_visible() {
        let mut low = study("low", vec![100, 100], -500);
        low.replicates = 1;
        let output = analyze_glioma_transportability(&request(), &[low]).unwrap();
        assert_eq!(output.disposition, TransportabilityDisposition::Unresolved);
        assert!(output
            .uncertainty
            .iter()
            .any(|item| item.contains("replicate-floor")));
    }
}
