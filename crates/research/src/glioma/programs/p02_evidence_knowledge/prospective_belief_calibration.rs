//! Prospective calibration of glioma claim forecasts against observed preclinical outcomes.
//!
//! Forecasts are evaluated prospectively rather than rewarded for narrative agreement.  Supported
//! outcomes count as positive, negative/contradicted outcomes count as non-positive, and unknown
//! or unmeasured outcomes remain omitted.  Fixed-point metrics make the result reproducible across
//! language runtimes and suitable for frontier promotion.

use crate::glioma::evidence::EvidenceState;
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P02-F31";
pub const OUTPUT_SCHEMA: &str = "GliomaProspectiveBeliefCalibration1@1";
pub const MAX_OBSERVATIONS: usize = 100_000;
pub const MAX_CLAIMS: usize = 16_384;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BeliefForecastObservation {
    pub observation_id: String,
    pub claim_id: String,
    pub epoch: u32,
    pub predicted_support_milli: u16,
    pub observed_state: EvidenceState,
    pub weight_milli: u16,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProspectiveBeliefCalibrationRequest {
    pub objective: String,
    pub observations: Vec<BeliefForecastObservation>,
    pub min_observations: usize,
    pub drift_threshold_milli: u16,
    pub max_claims: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BeliefCalibrationClaimDisposition {
    Calibrated,
    Drifted,
    Contradicted,
    Insufficient,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProspectiveBeliefCalibrationDisposition {
    Calibrated,
    ReviewRequired,
    Partial,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BeliefCalibrationClaim {
    pub claim_id: String,
    pub observation_order: Vec<String>,
    pub eligible_observation_order: Vec<String>,
    pub omitted_observation_order: Vec<String>,
    pub eligible_observation_count: usize,
    pub mean_predicted_support_milli: u16,
    pub empirical_support_milli: u16,
    pub calibrated_support_milli: u16,
    pub brier_milli: u16,
    pub calibration_error_milli: u16,
    pub disposition: BeliefCalibrationClaimDisposition,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProspectiveBeliefCalibration {
    pub feature_id: String,
    pub output_schema: String,
    pub objective: String,
    pub claim_order: Vec<String>,
    pub claims: Vec<BeliefCalibrationClaim>,
    pub calibrated_claim_order: Vec<String>,
    pub drifted_claim_order: Vec<String>,
    pub contradicted_claim_order: Vec<String>,
    pub insufficient_claim_order: Vec<String>,
    pub omitted_observation_order: Vec<String>,
    pub uncertainty: Vec<String>,
    pub disposition: ProspectiveBeliefCalibrationDisposition,
    pub next_step: String,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ProspectiveBeliefCalibrationError {
    #[error("prospective belief calibration request is invalid: {0}")]
    InvalidRequest(String),
    #[error("prospective belief calibration output is invalid: {0}")]
    InvalidOutput(String),
    #[error("prospective belief calibration digest failed: {0}")]
    Digest(String),
}

fn canonical(values: &[String]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn digest_input(output: &ProspectiveBeliefCalibration) -> serde_json::Value {
    serde_json::json!({
        "feature_id": output.feature_id,
        "output_schema": output.output_schema,
        "objective": output.objective,
        "claim_order": output.claim_order,
        "claims": output.claims,
        "calibrated_claim_order": output.calibrated_claim_order,
        "drifted_claim_order": output.drifted_claim_order,
        "contradicted_claim_order": output.contradicted_claim_order,
        "insufficient_claim_order": output.insufficient_claim_order,
        "omitted_observation_order": output.omitted_observation_order,
        "uncertainty": output.uncertainty,
        "disposition": output.disposition,
        "next_step": output.next_step,
    })
}

impl ProspectiveBeliefCalibration {
    pub fn validate(&self) -> Result<(), ProspectiveBeliefCalibrationError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || self.objective.trim().is_empty()
            || !canonical(&self.claim_order)
            || !canonical(&self.calibrated_claim_order)
            || !canonical(&self.drifted_claim_order)
            || !canonical(&self.contradicted_claim_order)
            || !canonical(&self.insufficient_claim_order)
            || !canonical(&self.omitted_observation_order)
            || !canonical(&self.uncertainty)
            || self.claims.len() != self.claim_order.len()
            || self
                .claims
                .iter()
                .map(|claim| claim.claim_id.clone())
                .collect::<Vec<_>>()
                != self.claim_order
            || self.claims.iter().any(|claim| {
                claim.claim_id.trim().is_empty()
                    || !canonical(&claim.observation_order)
                    || !canonical(&claim.eligible_observation_order)
                    || !canonical(&claim.omitted_observation_order)
                    || claim.eligible_observation_count != claim.eligible_observation_order.len()
                    || claim.mean_predicted_support_milli > 1_000
                    || claim.empirical_support_milli > 1_000
                    || claim.calibrated_support_milli > 1_000
                    || claim.brier_milli > 1_000
                    || claim.calibration_error_milli > 1_000
            })
        {
            return Err(ProspectiveBeliefCalibrationError::InvalidOutput(
                "identity, canonical ordering, partitions, or calibration bounds are invalid"
                    .into(),
            ));
        }
        let mut partition = BTreeSet::new();
        let mut count = 0;
        for claim_id in self
            .calibrated_claim_order
            .iter()
            .chain(self.drifted_claim_order.iter())
            .chain(self.contradicted_claim_order.iter())
            .chain(self.insufficient_claim_order.iter())
        {
            partition.insert(claim_id.clone());
            count += 1;
        }
        if partition != self.claim_order.iter().cloned().collect() || count != partition.len() {
            return Err(ProspectiveBeliefCalibrationError::InvalidOutput(
                "claim disposition partitions are inconsistent".into(),
            ));
        }
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| ProspectiveBeliefCalibrationError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(ProspectiveBeliefCalibrationError::Digest(
                "prospective calibration digest does not match canonical content".into(),
            ));
        }
        Ok(())
    }
}

fn outcome_value(state: EvidenceState) -> Option<u32> {
    match state {
        EvidenceState::Supported => Some(1_000),
        EvidenceState::Negative | EvidenceState::Contradicted => Some(0),
        EvidenceState::Unknown | EvidenceState::Stale | EvidenceState::Unmeasured => None,
    }
}

/// Evaluate prospective claim probabilities against eligible observed outcomes.
pub fn calibrate_glioma_beliefs_prospectively(
    request: &ProspectiveBeliefCalibrationRequest,
) -> Result<ProspectiveBeliefCalibration, ProspectiveBeliefCalibrationError> {
    if request.objective.trim().is_empty()
        || request.observations.is_empty()
        || request.observations.len() > MAX_OBSERVATIONS
        || request.min_observations == 0
        || request.min_observations > MAX_OBSERVATIONS
        || request.drift_threshold_milli > 1_000
        || request.max_claims == 0
        || request.max_claims > MAX_CLAIMS
    {
        return Err(ProspectiveBeliefCalibrationError::InvalidRequest(
            "objective, observations, bounds, or thresholds are invalid".into(),
        ));
    }
    let mut observations = BTreeMap::<String, BeliefForecastObservation>::new();
    for observation in &request.observations {
        if observation.observation_id.trim().is_empty()
            || observation.claim_id.trim().is_empty()
            || observation.epoch == 0
            || observation.predicted_support_milli > 1_000
            || observation.weight_milli == 0
            || observations
                .insert(observation.observation_id.clone(), observation.clone())
                .is_some()
        {
            return Err(ProspectiveBeliefCalibrationError::InvalidRequest(
                "observations must have unique ids, epochs, positive weights, and bounded forecasts"
                    .into(),
            ));
        }
    }
    let claim_ids = observations
        .values()
        .map(|observation| observation.claim_id.clone())
        .collect::<BTreeSet<_>>();
    if claim_ids.len() > request.max_claims {
        return Err(ProspectiveBeliefCalibrationError::InvalidRequest(
            "claim count exceeds max_claims".into(),
        ));
    }
    let mut claims = Vec::new();
    let mut all_omitted = BTreeSet::new();
    let mut calibrated = BTreeSet::new();
    let mut drifted = BTreeSet::new();
    let mut contradicted = BTreeSet::new();
    let mut insufficient = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    for claim_id in claim_ids {
        let rows = observations
            .values()
            .filter(|observation| observation.claim_id == claim_id)
            .collect::<Vec<_>>();
        let observation_order = rows
            .iter()
            .map(|observation| observation.observation_id.clone())
            .collect::<BTreeSet<_>>();
        let mut eligible = Vec::new();
        let mut omitted = BTreeSet::new();
        for observation in &rows {
            if outcome_value(observation.observed_state).is_some() {
                eligible.push(*observation);
            } else {
                omitted.insert(observation.observation_id.clone());
                all_omitted.insert(observation.observation_id.clone());
            }
        }
        let eligible_order = eligible
            .iter()
            .map(|observation| observation.observation_id.clone())
            .collect::<BTreeSet<_>>();
        let total_weight = eligible
            .iter()
            .map(|observation| observation.weight_milli as u64)
            .sum::<u64>();
        let predicted_sum = eligible
            .iter()
            .map(|observation| {
                observation.predicted_support_milli as u64 * observation.weight_milli as u64
            })
            .sum::<u64>();
        let empirical_sum = eligible
            .iter()
            .map(|observation| {
                outcome_value(observation.observed_state).unwrap_or_default() as u64
                    * observation.weight_milli as u64
            })
            .sum::<u64>();
        let mean_predicted = if total_weight == 0 {
            0
        } else {
            (predicted_sum / total_weight).min(1_000) as u16
        };
        let empirical = if total_weight == 0 {
            0
        } else {
            (empirical_sum / total_weight).min(1_000) as u16
        };
        let brier = if total_weight == 0 {
            0
        } else {
            (eligible
                .iter()
                .map(|observation| {
                    let prediction = observation.predicted_support_milli as i64;
                    let outcome =
                        outcome_value(observation.observed_state).unwrap_or_default() as i64;
                    (prediction - outcome).unsigned_abs().pow(2) as u64
                        * observation.weight_milli as u64
                        / 1_000
                })
                .sum::<u64>()
                / total_weight)
                .min(1_000) as u16
        };
        let calibration_error = mean_predicted.abs_diff(empirical);
        let disposition = if eligible.len() < request.min_observations {
            insufficient.insert(claim_id.clone());
            uncertainty.insert(format!("{claim_id}:insufficient-eligible-observations"));
            BeliefCalibrationClaimDisposition::Insufficient
        } else if empirical == 0 {
            contradicted.insert(claim_id.clone());
            uncertainty.insert(format!("{claim_id}:no-positive-outcome-observed"));
            BeliefCalibrationClaimDisposition::Contradicted
        } else if calibration_error > request.drift_threshold_milli {
            drifted.insert(claim_id.clone());
            uncertainty.insert(format!("{claim_id}:calibration-drift"));
            BeliefCalibrationClaimDisposition::Drifted
        } else {
            calibrated.insert(claim_id.clone());
            BeliefCalibrationClaimDisposition::Calibrated
        };
        let calibrated_support = if eligible.len() >= request.min_observations {
            empirical
        } else {
            mean_predicted
        };
        claims.push(BeliefCalibrationClaim {
            claim_id,
            observation_order: observation_order.into_iter().collect(),
            eligible_observation_order: eligible_order.into_iter().collect(),
            omitted_observation_order: omitted.into_iter().collect(),
            eligible_observation_count: eligible.len(),
            mean_predicted_support_milli: mean_predicted,
            empirical_support_milli: empirical,
            calibrated_support_milli: calibrated_support,
            brier_milli: brier,
            calibration_error_milli: calibration_error,
            disposition,
        });
    }
    claims.sort_by(|left, right| left.claim_id.cmp(&right.claim_id));
    let claim_order = claims
        .iter()
        .map(|claim| claim.claim_id.clone())
        .collect::<Vec<_>>();
    let disposition = if claims.is_empty() {
        ProspectiveBeliefCalibrationDisposition::Blocked
    } else if !contradicted.is_empty() || !drifted.is_empty() {
        ProspectiveBeliefCalibrationDisposition::ReviewRequired
    } else if !insufficient.is_empty() {
        ProspectiveBeliefCalibrationDisposition::Partial
    } else {
        ProspectiveBeliefCalibrationDisposition::Calibrated
    };
    let next_step = match disposition {
        ProspectiveBeliefCalibrationDisposition::Calibrated => {
            "feed calibrated support into claim reconciliation and the closed-loop frontier".into()
        }
        ProspectiveBeliefCalibrationDisposition::ReviewRequired => {
            "route drifted or contradicted claims to explicit belief revision and discriminating actions".into()
        }
        ProspectiveBeliefCalibrationDisposition::Partial => {
            "collect additional prospective observations before promoting calibration-dependent claims".into()
        }
        ProspectiveBeliefCalibrationDisposition::Blocked => {
            "admit prospective observations before recalibrating the research frontier".into()
        }
    };
    let mut output = ProspectiveBeliefCalibration {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        objective: request.objective.clone(),
        claim_order,
        claims,
        calibrated_claim_order: calibrated.into_iter().collect(),
        drifted_claim_order: drifted.into_iter().collect(),
        contradicted_claim_order: contradicted.into_iter().collect(),
        insufficient_claim_order: insufficient.into_iter().collect(),
        omitted_observation_order: all_omitted.into_iter().collect(),
        uncertainty: uncertainty.into_iter().collect(),
        disposition,
        next_step,
        digest: ContentHash::of_bytes(b"placeholder"),
    };
    output.digest = ContentHash::of_value(&digest_input(&output))
        .map_err(|error| ProspectiveBeliefCalibrationError::Digest(error.to_string()))?;
    output.validate()?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn observation(id: &str, prediction: u16, state: EvidenceState) -> BeliefForecastObservation {
        BeliefForecastObservation {
            observation_id: id.into(),
            claim_id: "claim-egfr".into(),
            epoch: 1,
            predicted_support_milli: prediction,
            observed_state: state,
            weight_milli: 1_000,
        }
    }

    fn request(rows: Vec<BeliefForecastObservation>) -> ProspectiveBeliefCalibrationRequest {
        ProspectiveBeliefCalibrationRequest {
            objective: "calibrate".into(),
            observations: rows,
            min_observations: 2,
            drift_threshold_milli: 200,
            max_claims: 10,
        }
    }

    #[test]
    fn well_calibrated_positive_forecasts_are_retained() {
        let output = calibrate_glioma_beliefs_prospectively(&request(vec![
            observation("obs-a", 800, EvidenceState::Supported),
            observation("obs-b", 900, EvidenceState::Supported),
        ]))
        .unwrap();
        assert_eq!(
            output.disposition,
            ProspectiveBeliefCalibrationDisposition::Calibrated
        );
        assert_eq!(output.calibrated_claim_order, vec!["claim-egfr"]);
        output.validate().unwrap();
    }

    #[test]
    fn stale_observations_remain_omitted() {
        let output = calibrate_glioma_beliefs_prospectively(&request(vec![
            observation("obs-a", 800, EvidenceState::Supported),
            observation("obs-b", 900, EvidenceState::Stale),
        ]))
        .unwrap();
        assert_eq!(
            output.disposition,
            ProspectiveBeliefCalibrationDisposition::Partial
        );
        assert_eq!(output.omitted_observation_order, vec!["obs-b"]);
    }

    #[test]
    fn all_negative_outcomes_require_review() {
        let output = calibrate_glioma_beliefs_prospectively(&request(vec![
            observation("obs-a", 900, EvidenceState::Negative),
            observation("obs-b", 850, EvidenceState::Contradicted),
        ]))
        .unwrap();
        assert_eq!(
            output.disposition,
            ProspectiveBeliefCalibrationDisposition::ReviewRequired
        );
        assert_eq!(output.contradicted_claim_order, vec!["claim-egfr"]);
    }
}
