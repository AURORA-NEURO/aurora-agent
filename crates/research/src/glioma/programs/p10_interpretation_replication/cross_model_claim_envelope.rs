//! Cross-model, partially identified claim envelopes for preclinical glioma research.
//!
//! This composition consumes investigator-declared study-level effect intervals rather than raw
//! observations. It gives each represented model system equal weight, expands every interval by a
//! declared hidden-bias budget, and reports whether a claim is sign-stable, model-dependent,
//! practically null, or under-covered. It is a transportability diagnostic, not a new causal
//! estimator and never makes a clinical prediction.

use crate::glioma_engine::{GliomaModelSystem, LocalArtifactRef};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const COMPOSITION_ID: &str = "glioma-cross-model-claim-envelope";
pub const OUTPUT_SCHEMA: &str = "GliomaCrossModelClaimEnvelope1@1";
pub const MAX_ESTIMATES: usize = 2_048;
pub const MAX_SYSTEMS: usize = 128;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CrossModelStudyEstimate {
    pub estimate_id: String,
    pub study_id: String,
    pub model_system: GliomaModelSystem,
    pub independent_group: String,
    pub interval_low_milli: i64,
    pub interval_high_milli: i64,
    pub quality_milli: u16,
    pub uncertainty_milli: u32,
    pub artifact: LocalArtifactRef,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CrossModelClaimEnvelopeRequest {
    pub objective: String,
    pub claim_id: String,
    pub min_model_systems: usize,
    pub min_studies_per_system: usize,
    pub practical_effect_milli: u64,
    pub hidden_bias_budget_milli: u64,
    pub max_between_system_range_milli: u64,
    pub estimates: Vec<CrossModelStudyEstimate>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CrossModelClaimEnvelopeDisposition {
    Qualified,
    Negative,
    ModelDependent,
    Partial,
    Unresolved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CrossModelSystemEnvelope {
    pub model_system: GliomaModelSystem,
    pub study_order: Vec<String>,
    pub estimate_count: usize,
    pub interval_low_milli: i64,
    pub interval_high_milli: i64,
    pub midpoint_milli: i64,
    pub hidden_bias_adjusted_low_milli: i64,
    pub hidden_bias_adjusted_high_milli: i64,
    pub independent_group_count: usize,
    pub leave_one_study_out_shift_milli: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CrossModelClaimEnvelope {
    pub composition_id: String,
    pub output_schema: String,
    pub objective: String,
    pub claim_id: String,
    pub model_system_order: Vec<GliomaModelSystem>,
    pub systems: Vec<CrossModelSystemEnvelope>,
    pub pooled_interval_low_milli: i64,
    pub pooled_interval_high_milli: i64,
    pub pooled_midpoint_milli: i64,
    pub between_system_range_milli: u64,
    pub sign_stable: bool,
    pub practical_effect_exceeded: bool,
    pub hidden_bias_budget_milli: u64,
    pub omitted_estimate_order: Vec<String>,
    pub next_action_order: Vec<String>,
    pub negative_evidence: Vec<String>,
    pub uncertainty: Vec<String>,
    pub disposition: CrossModelClaimEnvelopeDisposition,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum CrossModelClaimEnvelopeError {
    #[error("cross-model claim-envelope request is invalid: {0}")]
    InvalidRequest(String),
    #[error("cross-model claim-envelope output is invalid: {0}")]
    InvalidOutput(String),
    #[error("cross-model claim-envelope digest failed: {0}")]
    Digest(String),
}

fn canonical<T: Ord>(values: &[T]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn sorted_unique(mut values: Vec<String>) -> Vec<String> {
    values.sort();
    values.dedup();
    values
}

fn midpoint(low: i64, high: i64) -> i64 {
    ((low as i128 + high as i128) / 2).clamp(i64::MIN as i128, i64::MAX as i128) as i64
}

fn digest_input(output: &CrossModelClaimEnvelope) -> serde_json::Value {
    serde_json::json!({
        "composition_id": output.composition_id,
        "output_schema": output.output_schema,
        "objective": output.objective,
        "claim_id": output.claim_id,
        "model_system_order": output.model_system_order,
        "systems": output.systems,
        "pooled_interval_low_milli": output.pooled_interval_low_milli,
        "pooled_interval_high_milli": output.pooled_interval_high_milli,
        "pooled_midpoint_milli": output.pooled_midpoint_milli,
        "between_system_range_milli": output.between_system_range_milli,
        "sign_stable": output.sign_stable,
        "practical_effect_exceeded": output.practical_effect_exceeded,
        "hidden_bias_budget_milli": output.hidden_bias_budget_milli,
        "omitted_estimate_order": output.omitted_estimate_order,
        "next_action_order": output.next_action_order,
        "negative_evidence": output.negative_evidence,
        "uncertainty": output.uncertainty,
        "disposition": output.disposition,
    })
}

fn validate_request(
    request: &CrossModelClaimEnvelopeRequest,
) -> Result<(), CrossModelClaimEnvelopeError> {
    if request.objective.trim().is_empty()
        || request.claim_id.trim().is_empty()
        || request.min_model_systems == 0
        || request.min_model_systems > MAX_SYSTEMS
        || request.min_studies_per_system == 0
        || request.estimates.is_empty()
        || request.estimates.len() > MAX_ESTIMATES
    {
        return Err(CrossModelClaimEnvelopeError::InvalidRequest(
            "objective, claim, bounded system/study floors, and estimates are required".into(),
        ));
    }
    let mut estimate_ids = BTreeSet::new();
    for estimate in &request.estimates {
        if estimate.estimate_id.trim().is_empty()
            || estimate.study_id.trim().is_empty()
            || estimate.independent_group.trim().is_empty()
            || !estimate_ids.insert(estimate.estimate_id.clone())
            || estimate.interval_low_milli > estimate.interval_high_milli
            || estimate.quality_milli > 1_000
            || estimate.uncertainty_milli == 0
        {
            return Err(CrossModelClaimEnvelopeError::InvalidRequest(
                "estimates require unique ids, study/group identity, ordered intervals, bounded quality, and positive uncertainty".into(),
            ));
        }
        estimate
            .artifact
            .validate()
            .map_err(|error| CrossModelClaimEnvelopeError::InvalidRequest(error.to_string()))?;
    }
    Ok(())
}

impl CrossModelClaimEnvelope {
    pub fn validate(&self) -> Result<(), CrossModelClaimEnvelopeError> {
        if self.composition_id != COMPOSITION_ID
            || self.output_schema != OUTPUT_SCHEMA
            || self.objective.trim().is_empty()
            || self.claim_id.trim().is_empty()
            || !canonical(&self.model_system_order)
            || self.systems.len() != self.model_system_order.len()
            || self.systems.windows(2).any(|pair| {
                pair[0].model_system >= pair[1].model_system
                    || !canonical(&pair[0].study_order)
                    || !canonical(&pair[1].study_order)
            })
            || !canonical(&self.omitted_estimate_order)
            || !canonical(&self.next_action_order)
            || !canonical(&self.negative_evidence)
            || !canonical(&self.uncertainty)
            || self.pooled_interval_low_milli > self.pooled_interval_high_milli
        {
            return Err(CrossModelClaimEnvelopeError::InvalidOutput(
                "identity, ordering, system cardinality, interval, or evidence invariants are invalid".into(),
            ));
        }
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| CrossModelClaimEnvelopeError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(CrossModelClaimEnvelopeError::InvalidOutput(
                "claim envelope digest is not content-addressed".into(),
            ));
        }
        Ok(())
    }
}

/// Compile a robust cross-model effect envelope from independent study-level intervals.
pub fn analyze_glioma_cross_model_claim_envelope(
    request: &CrossModelClaimEnvelopeRequest,
) -> Result<CrossModelClaimEnvelope, CrossModelClaimEnvelopeError> {
    validate_request(request)?;
    let mut grouped = BTreeMap::<GliomaModelSystem, Vec<&CrossModelStudyEstimate>>::new();
    for estimate in &request.estimates {
        grouped
            .entry(estimate.model_system)
            .or_default()
            .push(estimate);
    }
    let mut systems = Vec::new();
    let mut omitted = Vec::new();
    let mut uncertainty = Vec::new();
    for (model_system, mut estimates) in grouped {
        estimates.sort_by(|left, right| {
            left.study_id
                .cmp(&right.study_id)
                .then_with(|| left.estimate_id.cmp(&right.estimate_id))
        });
        let mut studies = estimates
            .iter()
            .map(|estimate| estimate.study_id.clone())
            .collect::<Vec<_>>();
        studies.sort();
        studies.dedup();
        if studies.len() < request.min_studies_per_system {
            omitted.extend(
                estimates
                    .iter()
                    .map(|estimate| estimate.estimate_id.clone()),
            );
            uncertainty.push(format!(
                "model system {:?} has {} independent studies below required {}",
                model_system,
                studies.len(),
                request.min_studies_per_system
            ));
            continue;
        }
        let adjusted = estimates
            .iter()
            .map(|estimate| {
                (
                    estimate.interval_low_milli.saturating_sub(
                        request.hidden_bias_budget_milli.min(i64::MAX as u64) as i64,
                    ),
                    estimate.interval_high_milli.saturating_add(
                        request.hidden_bias_budget_milli.min(i64::MAX as u64) as i64,
                    ),
                    estimate.estimate_id.clone(),
                )
            })
            .collect::<Vec<_>>();
        let low = adjusted.iter().map(|item| item.0).sum::<i64>() / adjusted.len() as i64;
        let high = adjusted.iter().map(|item| item.1).sum::<i64>() / adjusted.len() as i64;
        let center = midpoint(low, high);
        let leave_one_shift = if adjusted.len() > 1 {
            let full = center as i128;
            adjusted
                .iter()
                .map(|item| {
                    let remaining = adjusted
                        .iter()
                        .filter(|other| other.2 != item.2)
                        .collect::<Vec<_>>();
                    let rem_low = remaining.iter().map(|other| other.0 as i128).sum::<i128>()
                        / remaining.len() as i128;
                    let rem_high = remaining.iter().map(|other| other.1 as i128).sum::<i128>()
                        / remaining.len() as i128;
                    (midpoint(rem_low as i64, rem_high as i64) as i128 - full).unsigned_abs() as i64
                })
                .max()
                .unwrap_or(0)
        } else {
            0
        };
        let groups = estimates
            .iter()
            .map(|estimate| estimate.independent_group.clone())
            .collect::<BTreeSet<_>>();
        systems.push(CrossModelSystemEnvelope {
            model_system,
            study_order: studies,
            estimate_count: estimates.len(),
            interval_low_milli: low,
            interval_high_milli: high,
            midpoint_milli: center,
            hidden_bias_adjusted_low_milli: adjusted.iter().map(|item| item.0).min().unwrap_or(low),
            hidden_bias_adjusted_high_milli: adjusted
                .iter()
                .map(|item| item.1)
                .max()
                .unwrap_or(high),
            independent_group_count: groups.len(),
            leave_one_study_out_shift_milli: leave_one_shift,
        });
    }
    systems.sort_by_key(|system| system.model_system);
    let system_set = systems
        .iter()
        .map(|system| system.model_system)
        .collect::<BTreeSet<_>>();
    let centers = systems
        .iter()
        .map(|system| system.midpoint_milli)
        .collect::<Vec<_>>();
    let pooled_midpoint = if centers.is_empty() {
        0
    } else {
        (centers.iter().map(|value| *value as i128).sum::<i128>() / centers.len() as i128) as i64
    };
    let pooled_low = systems
        .iter()
        .map(|system| system.hidden_bias_adjusted_low_milli)
        .min()
        .unwrap_or(0);
    let pooled_high = systems
        .iter()
        .map(|system| system.hidden_bias_adjusted_high_milli)
        .max()
        .unwrap_or(0);
    let between_range = centers
        .iter()
        .min()
        .zip(centers.iter().max())
        .map(|(low, high)| high.saturating_sub(*low).unsigned_abs())
        .unwrap_or(0);
    let positive = pooled_low > request.practical_effect_milli.min(i64::MAX as u64) as i64;
    let negative = pooled_high < -(request.practical_effect_milli.min(i64::MAX as u64) as i64);
    let sign_stable = positive || negative;
    let practical_exceeded = pooled_low
        > request.practical_effect_milli.min(i64::MAX as u64) as i64
        || pooled_high < -(request.practical_effect_milli.min(i64::MAX as u64) as i64);
    let mut negative_evidence = Vec::new();
    let mut next_action = Vec::new();
    if between_range > request.max_between_system_range_milli {
        negative_evidence.push(format!("between-system-range-exceeds-gate:{between_range}"));
        next_action.push("replicate-discordant-model-systems".into());
    }
    if !sign_stable {
        next_action.push("acquire-independent-study-or-wider-model-system-coverage".into());
    }
    if !omitted.is_empty() {
        next_action.push("resolve-undercovered-model-system-studies".into());
    }
    if systems.len() < request.min_model_systems {
        uncertainty.push(format!(
            "represented model systems {} below required {}",
            systems.len(),
            request.min_model_systems
        ));
    }
    let disposition = if systems.len() < request.min_model_systems || !omitted.is_empty() {
        CrossModelClaimEnvelopeDisposition::Partial
    } else if between_range > request.max_between_system_range_milli || !sign_stable {
        CrossModelClaimEnvelopeDisposition::ModelDependent
    } else if !practical_exceeded {
        CrossModelClaimEnvelopeDisposition::Negative
    } else {
        CrossModelClaimEnvelopeDisposition::Qualified
    };
    let mut output = CrossModelClaimEnvelope {
        composition_id: COMPOSITION_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        objective: request.objective.clone(),
        claim_id: request.claim_id.clone(),
        model_system_order: system_set.into_iter().collect(),
        systems,
        pooled_interval_low_milli: pooled_low,
        pooled_interval_high_milli: pooled_high,
        pooled_midpoint_milli: pooled_midpoint,
        between_system_range_milli: between_range,
        sign_stable,
        practical_effect_exceeded: practical_exceeded,
        hidden_bias_budget_milli: request.hidden_bias_budget_milli,
        omitted_estimate_order: sorted_unique(omitted),
        next_action_order: sorted_unique(next_action),
        negative_evidence: sorted_unique(negative_evidence),
        uncertainty: sorted_unique(uncertainty),
        disposition,
        digest: ContentHash::of_bytes(b"unsealed-glioma-cross-model-claim-envelope"),
    };
    output.digest = ContentHash::of_value(&digest_input(&output))
        .map_err(|error| CrossModelClaimEnvelopeError::Digest(error.to_string()))?;
    output.validate()?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn estimate(
        id: &str,
        study: &str,
        system: GliomaModelSystem,
        low: i64,
        high: i64,
    ) -> CrossModelStudyEstimate {
        CrossModelStudyEstimate {
            estimate_id: id.into(),
            study_id: study.into(),
            model_system: system,
            independent_group: format!("group-{study}"),
            interval_low_milli: low,
            interval_high_milli: high,
            quality_milli: 900,
            uncertainty_milli: 100,
            artifact: LocalArtifactRef {
                artifact_id: format!("artifact-{id}"),
                content_hash: ContentHash::of_bytes(id.as_bytes()),
                content_type: "application/json".into(),
                local_only: true,
                contains_human_data: false,
                contains_direct_identifiers: false,
            },
        }
    }

    fn request() -> CrossModelClaimEnvelopeRequest {
        CrossModelClaimEnvelopeRequest {
            objective: "test invasion mechanism transport".into(),
            claim_id: "invasion-claim".into(),
            min_model_systems: 2,
            min_studies_per_system: 2,
            practical_effect_milli: 50,
            hidden_bias_budget_milli: 10,
            max_between_system_range_milli: 200,
            estimates: vec![
                estimate("o1", "study-o1", GliomaModelSystem::Organoid, 200, 400),
                estimate("o2", "study-o2", GliomaModelSystem::Organoid, 220, 420),
                estimate("i1", "study-i1", GliomaModelSystem::InSilico, 180, 360),
                estimate("i2", "study-i2", GliomaModelSystem::InSilico, 190, 370),
            ],
        }
    }

    #[test]
    fn claim_envelope_is_sign_stable_across_model_systems() {
        let output = analyze_glioma_cross_model_claim_envelope(&request()).unwrap();
        assert_eq!(
            output.disposition,
            CrossModelClaimEnvelopeDisposition::Qualified
        );
        assert!(output.sign_stable);
        assert!(output.practical_effect_exceeded);
        output.validate().unwrap();
    }

    #[test]
    fn claim_envelope_preserves_undercovered_system_as_partial() {
        let mut request = request();
        request
            .estimates
            .retain(|estimate| estimate.model_system == GliomaModelSystem::Organoid);
        let output = analyze_glioma_cross_model_claim_envelope(&request).unwrap();
        assert_eq!(
            output.disposition,
            CrossModelClaimEnvelopeDisposition::Partial
        );
        assert!(!output.uncertainty.is_empty());
        output.validate().unwrap();
    }
}
