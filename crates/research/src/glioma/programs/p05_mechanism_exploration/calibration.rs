//! Preclinical mechanism-probability calibration and promotion gates.
//!
//! Discrimination can rank a mechanism, but ranking alone does not tell a research team whether
//! its probabilities are trustworthy over time. This feature scores held-out, typed local
//! observations with deterministic reliability bins and a prequential split. It never refits a
//! model, converts calibration into causality, or moves raw data out of the institution.

use crate::glioma_engine::{GliomaModelSystem, LocalArtifactRef};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P05-F21";
pub const OUTPUT_SCHEMA: &str = "GliomaMechanismCalibration1@1";
pub const MAX_MECHANISMS: usize = 256;
pub const MAX_OBSERVATIONS: usize = 65_536;
pub const MAX_ROUNDS: usize = 4_096;
pub const BIN_COUNT: usize = 10;
pub const MAX_VALUE_MILLI: u64 = 1_000_000;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MechanismCalibrationRequest {
    pub objective: String,
    pub model_system: GliomaModelSystem,
    pub min_observations_per_mechanism: usize,
    pub max_mechanisms: usize,
    pub max_rounds: usize,
    pub max_calibration_error_milli: u64,
    pub max_brier_loss_milli: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MechanismCalibrationObservation {
    pub round_index: usize,
    pub mechanism_id: String,
    pub feature_id: String,
    pub predicted_milli: u64,
    pub observed_milli: u64,
    pub uncertainty_milli: u64,
    pub artifact: LocalArtifactRef,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MechanismCalibrationBin {
    pub mechanism_id: String,
    pub bin_index: usize,
    pub lower_milli: u64,
    pub upper_milli: u64,
    pub observation_count: usize,
    pub mean_predicted_milli: u64,
    pub mean_observed_milli: u64,
    pub absolute_gap_milli: u64,
    pub brier_loss_milli: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MechanismCalibrationScoreDisposition {
    Qualified,
    Partial,
    Unresolved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MechanismCalibrationScore {
    pub mechanism_id: String,
    pub observation_count: usize,
    pub round_order: Vec<usize>,
    pub calibration_error_milli: u64,
    pub brier_loss_milli: u64,
    pub sharpness_milli: u64,
    pub coverage_milli: u16,
    pub held_out_observation_count: usize,
    pub disposition: MechanismCalibrationScoreDisposition,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MechanismCalibration {
    pub feature_id: String,
    pub output_schema: String,
    pub objective: String,
    pub model_system: GliomaModelSystem,
    pub mechanism_order: Vec<String>,
    pub scores: Vec<MechanismCalibrationScore>,
    pub bins: Vec<MechanismCalibrationBin>,
    pub negative_evidence: Vec<String>,
    pub uncertainty: Vec<String>,
    pub prequential_holdout_round: usize,
    pub disposition: MechanismCalibrationScoreDisposition,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum MechanismCalibrationError {
    #[error("mechanism calibration request is invalid: {0}")]
    InvalidRequest(String),
    #[error("mechanism calibration input is invalid: {0}")]
    InvalidInput(String),
    #[error("mechanism calibration output is invalid: {0}")]
    InvalidOutput(String),
    #[error("mechanism calibration digest failed: {0}")]
    Digest(String),
}

fn digest_input(output: &MechanismCalibration) -> serde_json::Value {
    serde_json::json!({
        "feature_id": output.feature_id,
        "output_schema": output.output_schema,
        "objective": output.objective,
        "model_system": output.model_system,
        "mechanism_order": output.mechanism_order,
        "scores": output.scores,
        "bins": output.bins,
        "negative_evidence": output.negative_evidence,
        "uncertainty": output.uncertainty,
        "prequential_holdout_round": output.prequential_holdout_round,
        "disposition": output.disposition,
    })
}

fn validate_request(
    request: &MechanismCalibrationRequest,
) -> Result<(), MechanismCalibrationError> {
    if request.objective.trim().is_empty()
        || request.min_observations_per_mechanism == 0
        || request.min_observations_per_mechanism > MAX_OBSERVATIONS
        || request.max_mechanisms == 0
        || request.max_mechanisms > MAX_MECHANISMS
        || request.max_rounds == 0
        || request.max_rounds > MAX_ROUNDS
        || request.max_calibration_error_milli > MAX_VALUE_MILLI
        || request.max_brier_loss_milli > MAX_VALUE_MILLI
    {
        return Err(MechanismCalibrationError::InvalidRequest(
            "objective, observation, mechanism, round, and metric bounds are invalid".into(),
        ));
    }
    Ok(())
}

impl MechanismCalibration {
    pub fn validate(&self) -> Result<(), MechanismCalibrationError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || self.objective.trim().is_empty()
            || self
                .mechanism_order
                .windows(2)
                .any(|pair| pair[0] >= pair[1])
            || self
                .scores
                .windows(2)
                .any(|pair| pair[0].mechanism_id >= pair[1].mechanism_id)
            || self.bins.windows(2).any(|pair| {
                (pair[0].mechanism_id.clone(), pair[0].bin_index)
                    >= (pair[1].mechanism_id.clone(), pair[1].bin_index)
            })
            || self.scores.iter().any(|score| {
                score.calibration_error_milli > MAX_VALUE_MILLI
                    || score.brier_loss_milli > MAX_VALUE_MILLI
                    || score.sharpness_milli > MAX_VALUE_MILLI
                    || score.coverage_milli > 1_000
                    || score.round_order.windows(2).any(|pair| pair[0] >= pair[1])
            })
            || self.bins.iter().any(|bin| {
                bin.bin_index >= BIN_COUNT
                    || bin.lower_milli >= bin.upper_milli
                    || bin.upper_milli > MAX_VALUE_MILLI
                    || bin.observation_count == 0
                    || bin.mean_predicted_milli > MAX_VALUE_MILLI
                    || bin.mean_observed_milli > MAX_VALUE_MILLI
                    || bin.absolute_gap_milli > MAX_VALUE_MILLI
                    || bin.brier_loss_milli > MAX_VALUE_MILLI
            })
            || self
                .negative_evidence
                .windows(2)
                .any(|pair| pair[0] >= pair[1])
            || self.uncertainty.windows(2).any(|pair| pair[0] >= pair[1])
        {
            return Err(MechanismCalibrationError::InvalidOutput(
                "identity, bounds, or canonical ordering is invalid".into(),
            ));
        }
        let mechanism_ids = self
            .mechanism_order
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>();
        let score_ids = self
            .scores
            .iter()
            .map(|score| score.mechanism_id.clone())
            .collect::<BTreeSet<_>>();
        if mechanism_ids != score_ids
            || self
                .bins
                .iter()
                .any(|bin| !mechanism_ids.contains(&bin.mechanism_id))
        {
            return Err(MechanismCalibrationError::InvalidOutput(
                "mechanism, score, and bin sets do not agree".into(),
            ));
        }
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| MechanismCalibrationError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(MechanismCalibrationError::InvalidOutput(
                "calibration digest is not content-addressed".into(),
            ));
        }
        Ok(())
    }
}

fn metric_gap(left: u64, right: u64) -> u64 {
    left.abs_diff(right)
}

fn mean(values: &[u64]) -> u64 {
    if values.is_empty() {
        return 0;
    }
    values.iter().copied().sum::<u64>() / values.len() as u64
}

/// Score mechanism probabilities against future local observations using fixed reliability bins.
/// The final round is reported as a prequential holdout; no observation is imputed or refit.
pub fn calibrate_glioma_mechanisms(
    request: &MechanismCalibrationRequest,
    observations: &[MechanismCalibrationObservation],
) -> Result<MechanismCalibration, MechanismCalibrationError> {
    validate_request(request)?;
    if observations.is_empty() || observations.len() > MAX_OBSERVATIONS {
        return Err(MechanismCalibrationError::InvalidInput(
            "observations must be non-empty and bounded".into(),
        ));
    }
    let mut canonical = observations.to_vec();
    canonical.sort_by(|left, right| {
        (
            left.mechanism_id.as_str(),
            left.round_index,
            left.feature_id.as_str(),
        )
            .cmp(&(
                right.mechanism_id.as_str(),
                right.round_index,
                right.feature_id.as_str(),
            ))
    });
    if canonical.windows(2).any(|pair| {
        (
            pair[0].mechanism_id.as_str(),
            pair[0].round_index,
            pair[0].feature_id.as_str(),
        ) == (
            pair[1].mechanism_id.as_str(),
            pair[1].round_index,
            pair[1].feature_id.as_str(),
        )
    }) {
        return Err(MechanismCalibrationError::InvalidInput(
            "duplicate mechanism-round-feature observations are not admissible".into(),
        ));
    }
    let mechanisms = canonical
        .iter()
        .map(|observation| observation.mechanism_id.clone())
        .collect::<BTreeSet<_>>();
    if mechanisms.is_empty() || mechanisms.len() > request.max_mechanisms {
        return Err(MechanismCalibrationError::InvalidInput(
            "mechanism count exceeds configured bound".into(),
        ));
    }
    if canonical.iter().any(|observation| {
        observation.mechanism_id.trim().is_empty()
            || observation.feature_id.trim().is_empty()
            || observation.round_index >= request.max_rounds
            || observation.predicted_milli > MAX_VALUE_MILLI
            || observation.observed_milli > MAX_VALUE_MILLI
            || observation.uncertainty_milli == 0
            || observation.artifact.artifact_id.trim().is_empty()
            || observation.artifact.contains_human_data
            || observation.artifact.contains_direct_identifiers
            || !observation.artifact.local_only
    }) {
        return Err(MechanismCalibrationError::InvalidInput(
            "observations require bounded value-only local artifacts".into(),
        ));
    }
    let max_round = canonical
        .iter()
        .map(|observation| observation.round_index)
        .max()
        .unwrap_or(0);
    let holdout_round = max_round;
    let mut by_mechanism = BTreeMap::<String, Vec<&MechanismCalibrationObservation>>::new();
    for observation in &canonical {
        by_mechanism
            .entry(observation.mechanism_id.clone())
            .or_default()
            .push(observation);
    }
    let mut bins = Vec::new();
    let mut scores = Vec::new();
    let mut negative_evidence = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    for (mechanism_id, records) in &by_mechanism {
        let round_order = records
            .iter()
            .map(|observation| observation.round_index)
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        let brier_loss_milli = mean(
            &records
                .iter()
                .map(|observation| {
                    let delta = observation
                        .predicted_milli
                        .abs_diff(observation.observed_milli);
                    delta.saturating_mul(delta) / MAX_VALUE_MILLI
                })
                .collect::<Vec<_>>(),
        );
        let sharpness_milli = mean(
            &records
                .iter()
                .map(|observation| {
                    let centered = observation.predicted_milli.abs_diff(500_000);
                    centered.saturating_mul(2)
                })
                .collect::<Vec<_>>(),
        );
        let held_out_count = records
            .iter()
            .filter(|observation| observation.round_index == holdout_round)
            .count();
        for bin_index in 0..BIN_COUNT {
            let lower = bin_index as u64 * 100_000;
            let upper = if bin_index + 1 == BIN_COUNT {
                MAX_VALUE_MILLI
            } else {
                lower + 100_000
            };
            let members = records
                .iter()
                .filter(|observation| {
                    observation.predicted_milli >= lower
                        && (observation.predicted_milli < upper
                            || (bin_index + 1 == BIN_COUNT && observation.predicted_milli <= upper))
                })
                .collect::<Vec<_>>();
            if members.is_empty() {
                continue;
            }
            let predicted = mean(
                &members
                    .iter()
                    .map(|observation| observation.predicted_milli)
                    .collect::<Vec<_>>(),
            );
            let observed = mean(
                &members
                    .iter()
                    .map(|observation| observation.observed_milli)
                    .collect::<Vec<_>>(),
            );
            bins.push(MechanismCalibrationBin {
                mechanism_id: mechanism_id.clone(),
                bin_index,
                lower_milli: lower,
                upper_milli: upper,
                observation_count: members.len(),
                mean_predicted_milli: predicted,
                mean_observed_milli: observed,
                absolute_gap_milli: metric_gap(predicted, observed),
                brier_loss_milli: mean(
                    &members
                        .iter()
                        .map(|observation| {
                            let delta = observation
                                .predicted_milli
                                .abs_diff(observation.observed_milli);
                            delta.saturating_mul(delta) / MAX_VALUE_MILLI
                        })
                        .collect::<Vec<_>>(),
                ),
            });
        }
        let calibration_error_milli = mean(
            &bins
                .iter()
                .filter(|bin| bin.mechanism_id == *mechanism_id)
                .map(|bin| bin.absolute_gap_milli)
                .collect::<Vec<_>>(),
        );
        let coverage_milli = ((records.len().min(request.min_observations_per_mechanism) * 1_000)
            / request.min_observations_per_mechanism) as u16;
        let score_disposition = if records.len() < request.min_observations_per_mechanism {
            uncertainty.insert(format!(
                "mechanism:{mechanism_id}:underpowered:{}",
                records.len()
            ));
            MechanismCalibrationScoreDisposition::Unresolved
        } else if calibration_error_milli > request.max_calibration_error_milli
            || brier_loss_milli > request.max_brier_loss_milli
        {
            negative_evidence.insert(format!(
                "mechanism:{mechanism_id}:calibration-gate-failed:error={calibration_error_milli}:brier={brier_loss_milli}"
            ));
            MechanismCalibrationScoreDisposition::Partial
        } else {
            MechanismCalibrationScoreDisposition::Qualified
        };
        for observation in records {
            if observation.predicted_milli >= 700_000 && observation.observed_milli <= 300_000 {
                negative_evidence.insert(format!(
                    "mechanism:{mechanism_id}:discordant:{}:{}",
                    observation.round_index, observation.feature_id
                ));
            }
            if observation.uncertainty_milli > 250_000 {
                uncertainty.insert(format!(
                    "mechanism:{mechanism_id}:high-uncertainty:{}:{}",
                    observation.round_index, observation.feature_id
                ));
            }
        }
        scores.push(MechanismCalibrationScore {
            mechanism_id: mechanism_id.clone(),
            observation_count: records.len(),
            round_order,
            calibration_error_milli,
            brier_loss_milli,
            sharpness_milli: sharpness_milli.min(MAX_VALUE_MILLI),
            coverage_milli,
            held_out_observation_count: held_out_count,
            disposition: score_disposition,
        });
    }
    bins.sort_by(|left, right| {
        (left.mechanism_id.as_str(), left.bin_index)
            .cmp(&(right.mechanism_id.as_str(), right.bin_index))
    });
    scores.sort_by(|left, right| left.mechanism_id.cmp(&right.mechanism_id));
    let mechanism_order = mechanisms.into_iter().collect::<Vec<_>>();
    let disposition = if scores
        .iter()
        .all(|score| score.disposition == MechanismCalibrationScoreDisposition::Qualified)
    {
        MechanismCalibrationScoreDisposition::Qualified
    } else if scores
        .iter()
        .any(|score| score.disposition == MechanismCalibrationScoreDisposition::Partial)
    {
        MechanismCalibrationScoreDisposition::Partial
    } else {
        MechanismCalibrationScoreDisposition::Unresolved
    };
    let mut output = MechanismCalibration {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        objective: request.objective.clone(),
        model_system: request.model_system,
        mechanism_order,
        scores,
        bins,
        negative_evidence: negative_evidence.into_iter().collect(),
        uncertainty: uncertainty.into_iter().collect(),
        prequential_holdout_round: holdout_round,
        disposition,
        digest: ContentHash::of_bytes(b"unsealed-glioma-mechanism-calibration"),
    };
    output.digest = ContentHash::of_value(&digest_input(&output))
        .map_err(|error| MechanismCalibrationError::Digest(error.to_string()))?;
    output.validate()?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    use bioprism_foundation::PRECLINICAL_BOUNDARY;
    use bioprism_ids::ContentHash;

    fn artifact(id: &str) -> LocalArtifactRef {
        LocalArtifactRef {
            artifact_id: id.into(),
            content_hash: ContentHash::of_bytes(id.as_bytes()),
            content_type: "application/vnd.aurora.glioma-calibration+json".into(),
            local_only: true,
            contains_human_data: false,
            contains_direct_identifiers: false,
        }
    }

    fn request() -> MechanismCalibrationRequest {
        MechanismCalibrationRequest {
            objective: "calibrate invasion mechanism probabilities".into(),
            model_system: GliomaModelSystem::Organoid,
            min_observations_per_mechanism: 2,
            max_mechanisms: 4,
            max_rounds: 8,
            max_calibration_error_milli: 200_000,
            max_brier_loss_milli: 200_000,
        }
    }

    #[test]
    fn calibration_is_prequential_and_replay_stable() {
        let observations = vec![
            MechanismCalibrationObservation {
                round_index: 0,
                mechanism_id: "motility".into(),
                feature_id: "f1".into(),
                predicted_milli: 800_000,
                observed_milli: 750_000,
                uncertainty_milli: 10_000,
                artifact: artifact("a1"),
            },
            MechanismCalibrationObservation {
                round_index: 1,
                mechanism_id: "motility".into(),
                feature_id: "f2".into(),
                predicted_milli: 700_000,
                observed_milli: 650_000,
                uncertainty_milli: 10_000,
                artifact: artifact("a2"),
            },
            MechanismCalibrationObservation {
                round_index: 0,
                mechanism_id: "matrix".into(),
                feature_id: "f1".into(),
                predicted_milli: 200_000,
                observed_milli: 250_000,
                uncertainty_milli: 10_000,
                artifact: artifact("a3"),
            },
            MechanismCalibrationObservation {
                round_index: 1,
                mechanism_id: "matrix".into(),
                feature_id: "f2".into(),
                predicted_milli: 300_000,
                observed_milli: 350_000,
                uncertainty_milli: 10_000,
                artifact: artifact("a4"),
            },
        ];
        let output = calibrate_glioma_mechanisms(&request(), &observations).unwrap();
        let replay = calibrate_glioma_mechanisms(
            &request(),
            &observations.into_iter().rev().collect::<Vec<_>>(),
        )
        .unwrap();
        assert_eq!(output, replay);
        assert_eq!(
            output.disposition,
            MechanismCalibrationScoreDisposition::Qualified
        );
        assert_eq!(output.prequential_holdout_round, 1);
        assert!(output
            .scores
            .iter()
            .all(|score| score.held_out_observation_count == 1));
    }

    #[test]
    fn calibration_keeps_underpowered_and_discordant_results_visible() {
        let mut request = request();
        request.min_observations_per_mechanism = 2;
        let observations = vec![MechanismCalibrationObservation {
            round_index: 0,
            mechanism_id: "motility".into(),
            feature_id: "f1".into(),
            predicted_milli: 900_000,
            observed_milli: 100_000,
            uncertainty_milli: 10_000,
            artifact: artifact("a1"),
        }];
        let output = calibrate_glioma_mechanisms(&request, &observations).unwrap();
        assert_eq!(
            output.disposition,
            MechanismCalibrationScoreDisposition::Unresolved
        );
        assert!(!output.negative_evidence.is_empty());
        assert!(!output.uncertainty.is_empty());
    }

    #[test]
    fn preclinical_boundary_fixture_is_constant() {
        assert!(PRECLINICAL_BOUNDARY.contains("no diagnosis"));
        assert!(PRECLINICAL_BOUNDARY.contains("no human-subject"));
    }
}
