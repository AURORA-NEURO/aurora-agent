//! Longitudinal finite-state mechanism filtering for preclinical glioma research.
//!
//! P05 already supports static mechanism discrimination and forward dynamics. This module adds
//! the inference layer between them: it updates a bounded mechanism-state posterior over ordered
//! local timepoints, accounts for transition priors and measurement uncertainty, marks missing or
//! contradictory features, and detects deterministic change points. It never infers a clinical
//! state and never treats a posterior as an observed biological fact.

use crate::glioma_engine::{GliomaModality, GliomaModelSystem, LocalArtifactRef};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P05-F16";
pub const OUTPUT_SCHEMA: &str = "GliomaMechanismStateFilter1@1";
pub const MAX_MECHANISMS: usize = 128;
pub const MAX_OBSERVATIONS: usize = 4_096;
pub const MAX_TIMEPOINTS: usize = 128;
pub const MAX_ABS_VALUE_MILLI: i64 = 1_000_000;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MechanismStateModel {
    pub mechanism_id: String,
    pub statement: String,
    pub prior_milli: u16,
    pub transition_milli_by_state: BTreeMap<String, u16>,
    pub predictions_milli: BTreeMap<String, i64>,
    pub process_uncertainty_milli: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MechanismStateObservation {
    pub timepoint: u16,
    pub feature_id: String,
    pub modality: GliomaModality,
    pub observed_milli: i64,
    pub measurement_uncertainty_milli: u64,
    pub artifact: LocalArtifactRef,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MechanismStateFilterRequest {
    pub objective: String,
    pub model_system: GliomaModelSystem,
    pub mechanisms: Vec<MechanismStateModel>,
    pub observations: Vec<MechanismStateObservation>,
    pub min_coverage_milli: u16,
    pub max_entropy_milli: u16,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MechanismStatePosterior {
    pub timepoint: u16,
    pub posterior_milli_by_mechanism: BTreeMap<String, u16>,
    pub dominant_mechanism: String,
    pub entropy_proxy_milli: u16,
    pub coverage_milli: u16,
    pub change_point: bool,
    pub negative_feature_order: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MechanismStateFilterDisposition {
    Qualified,
    Partial,
    Unresolved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MechanismStateFilterResult {
    pub feature_id: String,
    pub output_schema: String,
    pub objective: String,
    pub model_system: GliomaModelSystem,
    pub mechanism_order: Vec<String>,
    pub timepoint_order: Vec<u16>,
    pub posteriors: Vec<MechanismStatePosterior>,
    pub dominant_mechanism_order: Vec<String>,
    pub change_point_order: Vec<u16>,
    pub negative_evidence_order: Vec<String>,
    pub uncertainty_order: Vec<String>,
    pub disposition: MechanismStateFilterDisposition,
    pub next_action: String,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum MechanismStateFilterError {
    #[error("mechanism state filter request is invalid: {0}")]
    InvalidRequest(String),
    #[error("mechanism state filter observation is invalid: {0}")]
    InvalidObservation(String),
    #[error("mechanism state filter output is invalid: {0}")]
    InvalidOutput(String),
    #[error("mechanism state filter digest failed: {0}")]
    Digest(String),
}

fn canonical<T: Ord>(values: &[T]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn bounded_value(value: i64) -> bool {
    value.unsigned_abs() <= MAX_ABS_VALUE_MILLI as u64
}

fn digest_input(result: &MechanismStateFilterResult) -> serde_json::Value {
    serde_json::json!({
        "feature_id": result.feature_id,
        "output_schema": result.output_schema,
        "objective": result.objective,
        "model_system": result.model_system,
        "mechanism_order": result.mechanism_order,
        "timepoint_order": result.timepoint_order,
        "posteriors": result.posteriors,
        "dominant_mechanism_order": result.dominant_mechanism_order,
        "change_point_order": result.change_point_order,
        "negative_evidence_order": result.negative_evidence_order,
        "uncertainty_order": result.uncertainty_order,
        "disposition": result.disposition,
        "next_action": result.next_action,
    })
}

fn validate_request(
    request: &MechanismStateFilterRequest,
) -> Result<(), MechanismStateFilterError> {
    if request.objective.trim().is_empty()
        || request.mechanisms.is_empty()
        || request.mechanisms.len() > MAX_MECHANISMS
        || request.observations.is_empty()
        || request.observations.len() > MAX_OBSERVATIONS
        || request.min_coverage_milli > 1_000
        || request.max_entropy_milli > 1_000
    {
        return Err(MechanismStateFilterError::InvalidRequest(
            "objective, bounded mechanism/observation sets, observations, and coverage/entropy bounds are required".into(),
        ));
    }
    let mechanism_ids = request
        .mechanisms
        .iter()
        .map(|mechanism| mechanism.mechanism_id.clone())
        .collect::<BTreeSet<_>>();
    if mechanism_ids.len() != request.mechanisms.len()
        || request.mechanisms.iter().any(|mechanism| {
            mechanism.mechanism_id.trim().is_empty()
                || mechanism.statement.trim().is_empty()
                || mechanism.prior_milli == 0
                || mechanism.process_uncertainty_milli == 0
                || mechanism.predictions_milli.is_empty()
                || mechanism
                    .predictions_milli
                    .values()
                    .any(|value| !bounded_value(*value))
                || mechanism
                    .transition_milli_by_state
                    .iter()
                    .any(|(state, probability)| {
                        state.trim().is_empty()
                            || !mechanism_ids.contains(state)
                            || *probability > 1_000
                    })
                || mechanism.transition_milli_by_state.len() != mechanism_ids.len()
                || mechanism
                    .transition_milli_by_state
                    .values()
                    .map(|value| u32::from(*value))
                    .sum::<u32>()
                    != 1_000
        })
        || request
            .mechanisms
            .iter()
            .map(|mechanism| u32::from(mechanism.prior_milli))
            .sum::<u32>()
            != 1_000
    {
        return Err(MechanismStateFilterError::InvalidRequest(
            "mechanisms require unique ids, positive priors summing to 1000, complete transition rows, bounded predictions, and process uncertainty".into(),
        ));
    }
    let mut observation_keys = BTreeSet::new();
    for observation in &request.observations {
        if observation.feature_id.trim().is_empty()
            || observation.timepoint == 0
            || observation.measurement_uncertainty_milli == 0
            || !bounded_value(observation.observed_milli)
            || !observation.artifact.local_only
            || observation.artifact.contains_human_data
            || observation.artifact.contains_direct_identifiers
            || observation.artifact.validate().is_err()
            || !observation_keys.insert((observation.timepoint, observation.feature_id.clone()))
        {
            return Err(MechanismStateFilterError::InvalidObservation(
                "observations require unique positive timepoint/feature keys, bounded values, uncertainty, and valid local de-identified artifacts".into(),
            ));
        }
    }
    Ok(())
}

fn validate_output(result: &MechanismStateFilterResult) -> Result<(), MechanismStateFilterError> {
    if result.feature_id != FEATURE_ID
        || result.output_schema != OUTPUT_SCHEMA
        || result.objective.trim().is_empty()
        || result.mechanism_order.is_empty()
        || !canonical(&result.mechanism_order)
        || result.timepoint_order.is_empty()
        || !canonical(&result.timepoint_order)
        || result.posteriors.len() != result.timepoint_order.len()
        || result
            .posteriors
            .windows(2)
            .any(|pair| pair[0].timepoint >= pair[1].timepoint)
        || result.dominant_mechanism_order.len() != result.timepoint_order.len()
        || result
            .change_point_order
            .windows(2)
            .any(|pair| pair[0] >= pair[1])
        || !canonical(&result.negative_evidence_order)
        || !canonical(&result.uncertainty_order)
        || result.next_action.trim().is_empty()
        || result.posteriors.iter().any(|posterior| {
            posterior.timepoint == 0
                || posterior.posterior_milli_by_mechanism.len() != result.mechanism_order.len()
                || posterior
                    .posterior_milli_by_mechanism
                    .keys()
                    .cloned()
                    .collect::<Vec<_>>()
                    != result.mechanism_order
                || posterior
                    .posterior_milli_by_mechanism
                    .values()
                    .map(|value| u32::from(*value))
                    .sum::<u32>()
                    != 1_000
                || !result
                    .mechanism_order
                    .binary_search(&posterior.dominant_mechanism)
                    .is_ok()
                || posterior.entropy_proxy_milli > 1_000
                || posterior.coverage_milli > 1_000
                || !canonical(&posterior.negative_feature_order)
        })
    {
        return Err(MechanismStateFilterError::InvalidOutput(
            "identity, timepoint, posterior normalization, mechanism coverage, change-point, or uncertainty invariants are invalid".into(),
        ));
    }
    let expected = ContentHash::of_value(&digest_input(result))
        .map_err(|error| MechanismStateFilterError::Digest(error.to_string()))?;
    if expected != result.digest {
        return Err(MechanismStateFilterError::InvalidOutput(
            "mechanism state filter digest is not content-addressed".into(),
        ));
    }
    Ok(())
}

impl MechanismStateFilterResult {
    pub fn validate(&self) -> Result<(), MechanismStateFilterError> {
        validate_output(self)
    }
}

fn compatibility_milli(
    model: &MechanismStateModel,
    observation: &MechanismStateObservation,
) -> Option<u16> {
    let prediction = *model.predictions_milli.get(&observation.feature_id)?;
    let residual = (prediction - observation.observed_milli).unsigned_abs();
    let scale = model
        .process_uncertainty_milli
        .saturating_add(observation.measurement_uncertainty_milli)
        .max(1);
    let penalty = (residual.saturating_mul(1_000) / scale).min(1_000);
    Some((1_000_u64.saturating_sub(penalty)) as u16)
}

fn normalize(scores: &BTreeMap<String, u64>) -> BTreeMap<String, u16> {
    let total = scores.values().copied().sum::<u64>();
    if total == 0 {
        let share = 1_000_u16 / scores.len().max(1) as u16;
        return scores
            .keys()
            .enumerate()
            .map(|(index, key)| {
                let remainder = if index == 0 {
                    1_000_u16.saturating_sub(share.saturating_mul(scores.len() as u16))
                } else {
                    0
                };
                (key.clone(), share.saturating_add(remainder))
            })
            .collect();
    }
    let mut normalized = BTreeMap::new();
    let mut assigned = 0_u16;
    for (index, (key, score)) in scores.iter().enumerate() {
        let value = if index + 1 == scores.len() {
            1_000_u16.saturating_sub(assigned)
        } else {
            ((*score * 1_000) / total).min(1_000) as u16
        };
        assigned = assigned.saturating_add(value);
        normalized.insert(key.clone(), value);
    }
    normalized
}

/// Run a deterministic finite-state Bayesian filter over local longitudinal mechanism evidence.
pub fn filter_glioma_mechanism_states(
    request: &MechanismStateFilterRequest,
) -> Result<MechanismStateFilterResult, MechanismStateFilterError> {
    validate_request(request)?;
    let mut mechanisms = request.mechanisms.clone();
    mechanisms.sort_by_key(|mechanism| mechanism.mechanism_id.clone());
    let mechanism_order = mechanisms
        .iter()
        .map(|mechanism| mechanism.mechanism_id.clone())
        .collect::<Vec<_>>();
    let timepoints = request
        .observations
        .iter()
        .map(|observation| observation.timepoint)
        .collect::<BTreeSet<_>>();
    if timepoints.len() > MAX_TIMEPOINTS {
        return Err(MechanismStateFilterError::InvalidRequest(
            "observation timepoints exceed the bounded filter horizon".into(),
        ));
    }
    let observations_by_timepoint = request.observations.iter().fold(
        BTreeMap::<u16, Vec<&MechanismStateObservation>>::new(),
        |mut map, observation| {
            map.entry(observation.timepoint)
                .or_default()
                .push(observation);
            map
        },
    );
    let mut previous = mechanisms
        .iter()
        .map(|mechanism| {
            (
                mechanism.mechanism_id.clone(),
                u32::from(mechanism.prior_milli),
            )
        })
        .collect::<BTreeMap<_, _>>();
    let mut posteriors = Vec::new();
    let mut dominant_order = Vec::new();
    let mut change_points = Vec::new();
    let mut negative_evidence = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    let mut previous_dominant = None::<String>;
    for timepoint in timepoints.iter().copied() {
        let observations = observations_by_timepoint
            .get(&timepoint)
            .cloned()
            .unwrap_or_default();
        let mut predicted = BTreeMap::<String, u64>::new();
        for destination in &mechanisms {
            let score = mechanisms
                .iter()
                .map(|source| {
                    let transition = u64::from(
                        source
                            .transition_milli_by_state
                            .get(&destination.mechanism_id)
                            .copied()
                            .unwrap_or(0),
                    );
                    u64::from(*previous.get(&source.mechanism_id).unwrap_or(&0))
                        .saturating_mul(transition)
                        / 1_000
                })
                .sum::<u64>();
            predicted.insert(destination.mechanism_id.clone(), score);
        }
        let mut scores = BTreeMap::new();
        let mut negative_features = BTreeSet::new();
        for mechanism in &mechanisms {
            let compatibilities = observations
                .iter()
                .filter_map(|observation| {
                    compatibility_milli(mechanism, observation).map(|compatibility| {
                        if compatibility < 500 {
                            negative_features
                                .insert(format!("time-{}:{}", timepoint, observation.feature_id));
                        }
                        u64::from(compatibility)
                    })
                })
                .collect::<Vec<_>>();
            let compatibility = if compatibilities.is_empty() {
                1_000_u64
            } else {
                compatibilities.iter().sum::<u64>() / compatibilities.len() as u64
            };
            scores.insert(
                mechanism.mechanism_id.clone(),
                predicted
                    .get(&mechanism.mechanism_id)
                    .copied()
                    .unwrap_or(0)
                    .saturating_mul(compatibility),
            );
        }
        let posterior_u16 = normalize(&scores);
        let posterior = posterior_u16
            .iter()
            .map(|(mechanism, probability)| (mechanism.clone(), *probability))
            .collect::<BTreeMap<_, _>>();
        let dominant = posterior
            .iter()
            .max_by(|left, right| left.1.cmp(right.1).then_with(|| right.0.cmp(left.0)))
            .map(|(mechanism, _)| mechanism.clone())
            .ok_or_else(|| {
                MechanismStateFilterError::InvalidOutput(
                    "normalized posterior has no dominant mechanism".into(),
                )
            })?;
        let concentration = posterior
            .values()
            .map(|probability| {
                u64::from(*probability).saturating_mul(u64::from(*probability)) / 1_000
            })
            .sum::<u64>();
        let entropy_proxy = (1_000_u64.saturating_sub(concentration)).min(1_000) as u16;
        let coverage = if observations.is_empty() {
            0
        } else {
            let measured = observations
                .iter()
                .filter(|observation| {
                    mechanisms.iter().any(|mechanism| {
                        mechanism
                            .predictions_milli
                            .contains_key(&observation.feature_id)
                    })
                })
                .count();
            ((measured * 1_000) / observations.len()).min(1_000) as u16
        };
        let change_point = previous_dominant
            .as_ref()
            .is_some_and(|previous| previous != &dominant)
            || entropy_proxy > request.max_entropy_milli;
        if change_point {
            change_points.push(timepoint);
        }
        if coverage < request.min_coverage_milli {
            uncertainty.insert(format!("time-{}:coverage-{}", timepoint, coverage));
        }
        if entropy_proxy > request.max_entropy_milli {
            uncertainty.insert(format!("time-{}:entropy-{}", timepoint, entropy_proxy));
        }
        negative_evidence.extend(negative_features.iter().cloned());
        dominant_order.push(dominant.clone());
        posteriors.push(MechanismStatePosterior {
            timepoint,
            posterior_milli_by_mechanism: posterior.clone(),
            dominant_mechanism: dominant.clone(),
            entropy_proxy_milli: entropy_proxy,
            coverage_milli: coverage,
            change_point,
            negative_feature_order: negative_features.into_iter().collect(),
        });
        previous = posterior
            .iter()
            .map(|(mechanism, probability)| (mechanism.clone(), u32::from(*probability)))
            .collect();
        previous_dominant = Some(dominant);
    }
    let complete = posteriors
        .iter()
        .all(|posterior| posterior.coverage_milli >= request.min_coverage_milli)
        && posteriors
            .iter()
            .all(|posterior| posterior.entropy_proxy_milli <= request.max_entropy_milli);
    let disposition = if posteriors.is_empty() {
        MechanismStateFilterDisposition::Unresolved
    } else if complete {
        MechanismStateFilterDisposition::Qualified
    } else {
        MechanismStateFilterDisposition::Partial
    };
    let next_action = match disposition {
        MechanismStateFilterDisposition::Qualified => {
            "send the filtered mechanism trajectory to counterfactual and discriminating-action planning"
        }
        MechanismStateFilterDisposition::Partial => {
            "acquire missing or discriminating features at the uncertain timepoints before promoting the trajectory"
        }
        MechanismStateFilterDisposition::Unresolved => {
            "provide ordered local observations and typed mechanism transitions before filtering"
        }
    }
    .to_string();
    let mut result = MechanismStateFilterResult {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        objective: request.objective.clone(),
        model_system: request.model_system,
        mechanism_order,
        timepoint_order: timepoints.iter().copied().collect(),
        posteriors,
        dominant_mechanism_order: dominant_order,
        change_point_order: change_points,
        negative_evidence_order: negative_evidence.into_iter().collect(),
        uncertainty_order: uncertainty.into_iter().collect(),
        disposition,
        next_action,
        digest: ContentHash::of_bytes(b"unsealed-glioma-mechanism-state-filter"),
    };
    result.digest = ContentHash::of_value(&digest_input(&result))
        .map_err(|error| MechanismStateFilterError::Digest(error.to_string()))?;
    validate_output(&result)?;
    Ok(result)
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

    fn model(id: &str, prior: u16, prediction: i64) -> MechanismStateModel {
        MechanismStateModel {
            mechanism_id: id.into(),
            statement: format!("{id} explains invasion"),
            prior_milli: prior,
            transition_milli_by_state: BTreeMap::from([
                ("growth".into(), if id == "growth" { 900 } else { 100 }),
                ("stress".into(), if id == "stress" { 900 } else { 100 }),
            ]),
            predictions_milli: BTreeMap::from([("invasion-score".into(), prediction)]),
            process_uncertainty_milli: 100,
        }
    }

    fn request(observations: Vec<MechanismStateObservation>) -> MechanismStateFilterRequest {
        MechanismStateFilterRequest {
            objective: "track longitudinal glioma invasion mechanisms".into(),
            model_system: GliomaModelSystem::Organoid,
            mechanisms: vec![model("growth", 500, 800), model("stress", 500, 200)],
            observations,
            min_coverage_milli: 800,
            max_entropy_milli: 900,
        }
    }

    fn observation(timepoint: u16, value: i64, feature: &str) -> MechanismStateObservation {
        MechanismStateObservation {
            timepoint,
            feature_id: feature.into(),
            modality: GliomaModality::Imaging,
            observed_milli: value,
            measurement_uncertainty_milli: 100,
            artifact: artifact(&format!("artifact-{timepoint}-{feature}")),
        }
    }

    #[test]
    fn filter_tracks_dominant_mechanism_and_validates_posterior() {
        let result = filter_glioma_mechanism_states(&request(vec![
            observation(1, 790, "invasion-score"),
            observation(2, 780, "invasion-score"),
        ]))
        .expect("state filter");
        assert_eq!(result.posteriors.len(), 2);
        assert_eq!(result.posteriors[0].dominant_mechanism, "growth");
        assert!(result.change_point_order.is_empty());
        result.validate().expect("digest and invariants");
    }

    #[test]
    fn missing_feature_coverage_remains_uncertain() {
        let mut stress = model("stress", 500, 200);
        stress.predictions_milli = BTreeMap::from([("stress-score".into(), 200)]);
        let mut request = request(vec![observation(1, 500, "unknown-feature")]);
        request.mechanisms[1] = stress;
        let result = filter_glioma_mechanism_states(&request).expect("state filter");
        assert_eq!(result.disposition, MechanismStateFilterDisposition::Partial);
        assert!(result
            .uncertainty_order
            .iter()
            .any(|item| item.contains("coverage")));
    }

    #[test]
    fn shifted_observation_creates_change_point_and_negative_evidence() {
        let result = filter_glioma_mechanism_states(&request(vec![
            observation(1, 790, "invasion-score"),
            observation(2, 210, "invasion-score"),
        ]))
        .expect("state filter");
        assert!(result.change_point_order.contains(&2));
        assert!(result
            .negative_evidence_order
            .iter()
            .any(|item| item.contains("time-2")));
    }
}
