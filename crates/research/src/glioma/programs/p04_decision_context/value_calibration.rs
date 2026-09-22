//! Outcome calibration for the glioma value-of-information planner.
//!
//! The optimizer forecasts which local research actions are worth running. This module closes
//! the learning loop without allowing an autonomous caller to silently rewrite history: prior
//! forecasts are joined with typed outcomes from completed runs, shrunk toward the current
//! model, and returned with confidence, forecast error, negative outcomes, and conflict states.
//! Calibrated scores are still planning values; they are not biological evidence or clinical
//! conclusions.

use super::value_optimizer::{weighted_utility, DecisionValueCandidate, DecisionValueWeights};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P04-F14";
pub const OUTPUT_SCHEMA: &str = "GliomaDecisionValueCalibration1@1";
pub const MAX_CANDIDATES: usize = 256;
pub const MAX_OBSERVATIONS: usize = 2_048;
pub const MAX_RESULTS: usize = 256;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DecisionValueObservation {
    pub action_id: String,
    pub run_id: String,
    pub predicted_utility_milli: i32,
    pub observed_information_gain_milli: u16,
    pub observed_uncertainty_reduction_milli: u16,
    pub observed_contradiction_resolution_milli: u16,
    pub observed_reproducibility_milli: u16,
    pub failed: bool,
    pub sample_weight: u16,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DecisionValueCalibrationRequest {
    pub objective: String,
    pub candidates: Vec<DecisionValueCandidate>,
    pub observations: Vec<DecisionValueObservation>,
    pub weights: DecisionValueWeights,
    pub shrinkage_weight: u16,
    pub min_confidence_milli: u16,
    pub conflict_error_milli: u32,
    pub max_results: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DecisionValueCalibrationDisposition {
    Calibrated,
    PriorOnly,
    Conflicted,
    Unreliable,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DecisionValueCalibrationRecord {
    pub action_id: String,
    pub prior_utility_milli: i32,
    pub observed_utility_milli: Option<i32>,
    pub calibrated_utility_milli: i32,
    pub observation_weight: u32,
    pub mean_absolute_error_milli: u32,
    pub confidence_milli: u16,
    pub disposition: DecisionValueCalibrationDisposition,
    pub reason_order: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DecisionValueCalibrationCampaignDisposition {
    Ready,
    PriorOnly,
    Conflicted,
    Unresolved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DecisionValueCalibrationResult {
    pub feature_id: String,
    pub output_schema: String,
    pub objective: String,
    pub candidate_order: Vec<String>,
    pub ranking_order: Vec<String>,
    pub records: Vec<DecisionValueCalibrationRecord>,
    pub negative_evidence_order: Vec<String>,
    pub uncertainty_order: Vec<String>,
    pub disposition: DecisionValueCalibrationCampaignDisposition,
    pub next_action: String,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum DecisionValueCalibrationError {
    #[error("decision value calibration request is invalid: {0}")]
    InvalidRequest(String),
    #[error("decision value calibration observation is invalid: {0}")]
    InvalidObservation(String),
    #[error("decision value calibration output is invalid: {0}")]
    InvalidOutput(String),
    #[error("decision value calibration digest failed: {0}")]
    Digest(String),
}

fn canonical(values: &[String]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn bounded_metric(value: u16) -> bool {
    value <= 1_000
}

fn observed_utility(observation: &DecisionValueObservation, weights: &DecisionValueWeights) -> i32 {
    let positive = i64::from(observation.observed_information_gain_milli)
        .saturating_mul(i64::from(weights.information_gain))
        .saturating_add(
            i64::from(observation.observed_uncertainty_reduction_milli)
                .saturating_mul(i64::from(weights.uncertainty_reduction)),
        )
        .saturating_add(
            i64::from(observation.observed_contradiction_resolution_milli)
                .saturating_mul(i64::from(weights.contradiction_resolution)),
        )
        .saturating_add(
            i64::from(observation.observed_reproducibility_milli)
                .saturating_mul(i64::from(weights.reproducibility)),
        );
    let failure_penalty = if observation.failed {
        1_000_i64.saturating_mul(i64::from(weights.failure_penalty))
    } else {
        0
    };
    ((positive.saturating_sub(failure_penalty)) / 100)
        .clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32
}

fn digest_input(result: &DecisionValueCalibrationResult) -> serde_json::Value {
    serde_json::json!({
        "feature_id": result.feature_id,
        "output_schema": result.output_schema,
        "objective": result.objective,
        "candidate_order": result.candidate_order,
        "ranking_order": result.ranking_order,
        "records": result.records,
        "negative_evidence_order": result.negative_evidence_order,
        "uncertainty_order": result.uncertainty_order,
        "disposition": result.disposition,
        "next_action": result.next_action,
    })
}

fn validate_request(
    request: &DecisionValueCalibrationRequest,
) -> Result<(), DecisionValueCalibrationError> {
    let weights_sum = u32::from(request.weights.information_gain)
        + u32::from(request.weights.uncertainty_reduction)
        + u32::from(request.weights.contradiction_resolution)
        + u32::from(request.weights.reproducibility)
        + u32::from(request.weights.diversity)
        + u32::from(request.weights.failure_penalty);
    if request.objective.trim().is_empty()
        || request.candidates.is_empty()
        || request.candidates.len() > MAX_CANDIDATES
        || request.observations.len() > MAX_OBSERVATIONS
        || request.shrinkage_weight == 0
        || request.min_confidence_milli > 1_000
        || request.conflict_error_milli == 0
        || request.max_results == 0
        || usize::from(request.max_results) > MAX_RESULTS
        || weights_sum == 0
        || weights_sum > 10_000
    {
        return Err(DecisionValueCalibrationError::InvalidRequest(
            "objective, bounded candidate/observation sets, positive shrinkage and result bounds, and valid weights are required".into(),
        ));
    }
    let mut known = BTreeSet::new();
    for candidate in &request.candidates {
        if candidate.action_id.trim().is_empty()
            || candidate.claim_id.trim().is_empty()
            || candidate.cost_units == 0
            || candidate.diversity_group.trim().is_empty()
            || !known.insert(candidate.action_id.clone())
            || !candidate
                .depends_on
                .windows(2)
                .all(|pair| pair[0] < pair[1])
            || candidate.information_gain_milli > 1_000
            || candidate.uncertainty_reduction_milli > 1_000
            || candidate.contradiction_resolution_milli > 1_000
            || candidate.reproducibility_milli > 1_000
            || candidate.failure_probability_milli > 1_000
        {
            return Err(DecisionValueCalibrationError::InvalidRequest(
                "candidates require unique identities, bounded metrics, positive costs, and canonical dependencies".into(),
            ));
        }
    }
    for candidate in &request.candidates {
        if candidate.depends_on.iter().any(|dependency| {
            dependency.trim().is_empty()
                || dependency >= &candidate.action_id
                || !known.contains(dependency)
        }) {
            return Err(DecisionValueCalibrationError::InvalidRequest(
                "candidate dependencies must be known, unique, ordered, and acyclic".into(),
            ));
        }
    }
    for observation in &request.observations {
        if observation.action_id.trim().is_empty()
            || observation.run_id.trim().is_empty()
            || !known.contains(&observation.action_id)
            || observation.sample_weight == 0
            || observation.predicted_utility_milli.unsigned_abs() > 100_000
            || !bounded_metric(observation.observed_information_gain_milli)
            || !bounded_metric(observation.observed_uncertainty_reduction_milli)
            || !bounded_metric(observation.observed_contradiction_resolution_milli)
            || !bounded_metric(observation.observed_reproducibility_milli)
        {
            return Err(DecisionValueCalibrationError::InvalidObservation(
                "observations require known actions, run identities, positive weights, and bounded forecast/outcome metrics".into(),
            ));
        }
    }
    let mut observation_keys = BTreeSet::new();
    if request.observations.iter().any(|observation| {
        !observation_keys.insert((observation.action_id.clone(), observation.run_id.clone()))
    }) {
        return Err(DecisionValueCalibrationError::InvalidObservation(
            "each action/run observation pair must be unique".into(),
        ));
    }
    Ok(())
}

fn validate_output(
    result: &DecisionValueCalibrationResult,
) -> Result<(), DecisionValueCalibrationError> {
    if result.feature_id != FEATURE_ID
        || result.output_schema != OUTPUT_SCHEMA
        || result.objective.trim().is_empty()
        || !canonical(&result.candidate_order)
        || result.candidate_order.is_empty()
        || result.records.len() != result.candidate_order.len()
        || result
            .records
            .windows(2)
            .any(|pair| pair[0].action_id >= pair[1].action_id)
        || result.ranking_order.is_empty()
        || result.ranking_order.len() > result.candidate_order.len()
        || {
            let mut ranking_ids = BTreeSet::new();
            result
                .ranking_order
                .iter()
                .any(|action_id| !ranking_ids.insert(action_id))
        }
        || result
            .ranking_order
            .iter()
            .any(|action_id| !result.candidate_order.binary_search(action_id).is_ok())
        || !canonical(&result.negative_evidence_order)
        || !canonical(&result.uncertainty_order)
        || result.records.iter().any(|record| {
            record.action_id.trim().is_empty()
                || record.reason_order.is_empty()
                || !canonical(&record.reason_order)
                || record.confidence_milli > 1_000
        })
        || result.next_action.trim().is_empty()
    {
        return Err(DecisionValueCalibrationError::InvalidOutput(
            "identity, candidate coverage, ranking, evidence, confidence, or reason invariants are invalid".into(),
        ));
    }
    let expected = ContentHash::of_value(&digest_input(result))
        .map_err(|error| DecisionValueCalibrationError::Digest(error.to_string()))?;
    if expected != result.digest {
        return Err(DecisionValueCalibrationError::InvalidOutput(
            "calibration digest is not content-addressed".into(),
        ));
    }
    Ok(())
}

impl DecisionValueCalibrationResult {
    pub fn validate(&self) -> Result<(), DecisionValueCalibrationError> {
        validate_output(self)
    }
}

/// Calibrate forecast utility from typed local outcomes with shrinkage and explicit conflict
/// handling. This function only changes future planning scores; it never mutates observations.
pub fn calibrate_glioma_decision_value(
    request: &DecisionValueCalibrationRequest,
) -> Result<DecisionValueCalibrationResult, DecisionValueCalibrationError> {
    validate_request(request)?;
    let mut candidates = request.candidates.clone();
    candidates.sort_by_key(|candidate| candidate.action_id.clone());
    let mut observations = request.observations.clone();
    observations.sort_by(|left, right| {
        left.action_id
            .cmp(&right.action_id)
            .then_with(|| left.run_id.cmp(&right.run_id))
    });
    let observations_by_action = observations.into_iter().fold(
        BTreeMap::<String, Vec<DecisionValueObservation>>::new(),
        |mut grouped, observation| {
            grouped
                .entry(observation.action_id.clone())
                .or_default()
                .push(observation);
            grouped
        },
    );
    let mut records = Vec::with_capacity(candidates.len());
    let mut negative_evidence = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    for candidate in &candidates {
        let prior = weighted_utility(candidate, &request.weights);
        let observations = observations_by_action
            .get(&candidate.action_id)
            .cloned()
            .unwrap_or_default();
        let total_weight = observations
            .iter()
            .map(|observation| u32::from(observation.sample_weight))
            .sum::<u32>();
        let (observed, mean_absolute_error) = if total_weight == 0 {
            (None, 0_u32)
        } else {
            let weighted_sum = observations
                .iter()
                .map(|observation| {
                    i64::from(observed_utility(observation, &request.weights))
                        .saturating_mul(i64::from(observation.sample_weight))
                })
                .sum::<i64>();
            let mean = weighted_sum / i64::from(total_weight);
            let absolute_error = observations
                .iter()
                .map(|observation| {
                    i64::from(
                        (i64::from(observation.predicted_utility_milli)
                            - i64::from(observed_utility(observation, &request.weights)))
                        .unsigned_abs()
                        .min(u64::from(u32::MAX)) as u32,
                    )
                    .saturating_mul(i64::from(observation.sample_weight))
                })
                .sum::<i64>()
                / i64::from(total_weight);
            (
                Some(mean.clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32),
                absolute_error.min(i64::from(u32::MAX)) as u32,
            )
        };
        let denominator = u32::from(request.shrinkage_weight).saturating_add(total_weight);
        let calibrated = observed.map_or(prior, |observed| {
            ((i64::from(prior).saturating_mul(i64::from(request.shrinkage_weight))
                + i64::from(observed).saturating_mul(i64::from(total_weight)))
                / i64::from(denominator))
            .clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32
        });
        let confidence = if total_weight == 0 {
            0
        } else {
            let coverage =
                (1_000_u64 * u64::from(total_weight) / u64::from(denominator)).min(1_000);
            let error_penalty = (u64::from(mean_absolute_error) * 1_000
                / u64::from(request.conflict_error_milli.max(10_000)))
            .min(1_000);
            (coverage * (1_000 - error_penalty) / 1_000) as u16
        };
        let disposition = if total_weight == 0 {
            uncertainty.insert(format!("prior-only:{}", candidate.action_id));
            DecisionValueCalibrationDisposition::PriorOnly
        } else if mean_absolute_error >= request.conflict_error_milli {
            negative_evidence.insert(format!("forecast-conflict:{}", candidate.action_id));
            uncertainty.insert(format!("conflicted-calibration:{}", candidate.action_id));
            DecisionValueCalibrationDisposition::Conflicted
        } else if confidence < request.min_confidence_milli {
            uncertainty.insert(format!(
                "low-calibration-confidence:{}",
                candidate.action_id
            ));
            DecisionValueCalibrationDisposition::Unreliable
        } else {
            DecisionValueCalibrationDisposition::Calibrated
        };
        for observation in &observations {
            if observation.failed {
                negative_evidence.insert(format!(
                    "failed:{}:{}",
                    observation.action_id, observation.run_id
                ));
            }
        }
        let mut reasons = BTreeSet::new();
        match disposition {
            DecisionValueCalibrationDisposition::Calibrated => {
                reasons.insert("calibrated-from-local-outcomes".to_string());
            }
            DecisionValueCalibrationDisposition::PriorOnly => {
                reasons.insert("no-observation-prior".to_string());
            }
            DecisionValueCalibrationDisposition::Conflicted => {
                reasons.insert("forecast-error-conflict".to_string());
            }
            DecisionValueCalibrationDisposition::Unreliable => {
                reasons.insert("insufficient-calibration-confidence".to_string());
            }
        }
        if observations.iter().any(|observation| observation.failed) {
            reasons.insert("negative-outcome-observed".to_string());
        }
        records.push(DecisionValueCalibrationRecord {
            action_id: candidate.action_id.clone(),
            prior_utility_milli: prior,
            observed_utility_milli: observed,
            calibrated_utility_milli: calibrated,
            observation_weight: total_weight,
            mean_absolute_error_milli: mean_absolute_error,
            confidence_milli: confidence,
            disposition,
            reason_order: reasons.into_iter().collect(),
        });
    }
    let mut ranking = records.clone();
    ranking.sort_by(|left, right| {
        right
            .calibrated_utility_milli
            .cmp(&left.calibrated_utility_milli)
            .then_with(|| right.confidence_milli.cmp(&left.confidence_milli))
            .then_with(|| left.action_id.cmp(&right.action_id))
    });
    let ranking_order = ranking
        .iter()
        .take(usize::from(request.max_results))
        .map(|record| record.action_id.clone())
        .collect::<Vec<_>>();
    let disposition = if request.observations.is_empty() {
        DecisionValueCalibrationCampaignDisposition::PriorOnly
    } else if records.iter().any(|record| {
        matches!(
            record.disposition,
            DecisionValueCalibrationDisposition::Conflicted
                | DecisionValueCalibrationDisposition::Unreliable
        )
    }) {
        DecisionValueCalibrationCampaignDisposition::Conflicted
    } else if records.iter().all(|record| {
        matches!(
            record.disposition,
            DecisionValueCalibrationDisposition::PriorOnly
        )
    }) {
        DecisionValueCalibrationCampaignDisposition::PriorOnly
    } else {
        DecisionValueCalibrationCampaignDisposition::Ready
    };
    let next_action = match disposition {
        DecisionValueCalibrationCampaignDisposition::Ready => {
            "submit calibrated ranking to the bounded value-of-information optimizer"
        }
        DecisionValueCalibrationCampaignDisposition::PriorOnly => {
            "collect typed local outcomes before granting learned utility more authority"
        }
        DecisionValueCalibrationCampaignDisposition::Conflicted => {
            "review forecast conflicts and negative outcomes before recalibrating the next portfolio"
        }
        DecisionValueCalibrationCampaignDisposition::Unresolved => {
            "supply typed candidates and observations before producing a calibrated ranking"
        }
    }
    .to_string();
    let mut result = DecisionValueCalibrationResult {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        objective: request.objective.clone(),
        candidate_order: candidates
            .iter()
            .map(|candidate| candidate.action_id.clone())
            .collect(),
        ranking_order,
        records,
        negative_evidence_order: negative_evidence.into_iter().collect(),
        uncertainty_order: uncertainty.into_iter().collect(),
        disposition,
        next_action,
        digest: ContentHash::of_bytes(b"unsealed-glioma-decision-value-calibration"),
    };
    result.digest = ContentHash::of_value(&digest_input(&result))
        .map_err(|error| DecisionValueCalibrationError::Digest(error.to_string()))?;
    validate_output(&result)?;
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::glioma_engine::{GliomaModality, GliomaModelSystem};

    fn candidate(id: &str) -> DecisionValueCandidate {
        DecisionValueCandidate {
            action_id: id.into(),
            claim_id: "claim-invasion".into(),
            modality: GliomaModality::Imaging,
            model_system: GliomaModelSystem::Organoid,
            depends_on: Vec::new(),
            cost_units: 2,
            information_gain_milli: 700,
            uncertainty_reduction_milli: 700,
            contradiction_resolution_milli: 500,
            reproducibility_milli: 800,
            failure_probability_milli: 50,
            diversity_group: id.into(),
            available: true,
        }
    }

    fn request(
        candidates: Vec<DecisionValueCandidate>,
        observations: Vec<DecisionValueObservation>,
    ) -> DecisionValueCalibrationRequest {
        DecisionValueCalibrationRequest {
            objective: "calibrate the next glioma research portfolio from local outcomes".into(),
            candidates,
            observations,
            weights: DecisionValueWeights {
                information_gain: 30,
                uncertainty_reduction: 25,
                contradiction_resolution: 20,
                reproducibility: 15,
                diversity: 10,
                failure_penalty: 10,
            },
            shrinkage_weight: 10,
            min_confidence_milli: 100,
            conflict_error_milli: 3_000,
            max_results: 4,
        }
    }

    fn observation(action_id: &str, run_id: &str, failed: bool) -> DecisionValueObservation {
        DecisionValueObservation {
            action_id: action_id.into(),
            run_id: run_id.into(),
            predicted_utility_milli: 745,
            observed_information_gain_milli: 900,
            observed_uncertainty_reduction_milli: 800,
            observed_contradiction_resolution_milli: 700,
            observed_reproducibility_milli: 900,
            failed,
            sample_weight: 2,
        }
    }

    #[test]
    fn calibrates_local_outcomes_and_returns_ranked_candidates() {
        let result = calibrate_glioma_decision_value(&request(
            vec![candidate("a-imaging"), candidate("b-imaging")],
            vec![observation("a-imaging", "run-001", false)],
        ))
        .expect("calibration");
        assert_eq!(
            result.disposition,
            DecisionValueCalibrationCampaignDisposition::Ready
        );
        assert_eq!(result.ranking_order.len(), 2);
        assert_eq!(result.records[0].observation_weight, 2);
        result.validate().expect("digest and invariants");
    }

    #[test]
    fn prior_only_candidates_remain_uncertain() {
        let result =
            calibrate_glioma_decision_value(&request(vec![candidate("a-imaging")], Vec::new()))
                .expect("calibration");
        assert_eq!(
            result.disposition,
            DecisionValueCalibrationCampaignDisposition::PriorOnly
        );
        assert!(result
            .uncertainty_order
            .iter()
            .any(|item| item.contains("prior-only")));
    }

    #[test]
    fn forecast_conflict_and_negative_outcome_are_visible() {
        let mut failed = observation("a-imaging", "run-002", true);
        failed.predicted_utility_milli = 9_000;
        failed.observed_information_gain_milli = 0;
        failed.observed_uncertainty_reduction_milli = 0;
        failed.observed_contradiction_resolution_milli = 0;
        failed.observed_reproducibility_milli = 0;
        let result =
            calibrate_glioma_decision_value(&request(vec![candidate("a-imaging")], vec![failed]))
                .expect("calibration");
        assert_eq!(
            result.disposition,
            DecisionValueCalibrationCampaignDisposition::Conflicted
        );
        assert!(result
            .negative_evidence_order
            .iter()
            .any(|item| item.contains("failed:a-imaging:run-002")));
        assert!(result
            .uncertainty_order
            .iter()
            .any(|item| item.contains("conflicted-calibration")));
    }
}
