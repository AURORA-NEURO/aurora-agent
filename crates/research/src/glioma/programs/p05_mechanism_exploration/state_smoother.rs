//! Fixed-interval mechanism-state smoothing for preclinical glioma research.
//!
//! The state filter is useful while a study is running, but a completed longitudinal study also
//! needs a retrospective pass: later observations can explain an earlier latent transition. This
//! module implements a bounded forward-backward smoother over typed local observations. It reports
//! transition support, coverage, entropy, and negative features explicitly; it never converts a
//! smoothed mechanism state into a clinical conclusion or an executed intervention.

use super::state_filter::{MechanismStateModel, MechanismStateObservation};
use crate::glioma_engine::GliomaModelSystem;
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P05-F17";
pub const OUTPUT_SCHEMA: &str = "GliomaMechanismStateSmoother1@1";
pub const MAX_MECHANISMS: usize = 128;
pub const MAX_OBSERVATIONS: usize = 4_096;
pub const MAX_TIMEPOINTS: usize = 128;
pub const MAX_ABS_VALUE_MILLI: i64 = 1_000_000;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MechanismStateSmootherRequest {
    pub objective: String,
    pub model_system: GliomaModelSystem,
    pub mechanisms: Vec<MechanismStateModel>,
    pub observations: Vec<MechanismStateObservation>,
    pub min_coverage_milli: u16,
    pub max_entropy_milli: u16,
    pub min_transition_support_milli: u16,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MechanismStateSmoothPosterior {
    pub timepoint: u16,
    pub posterior_milli_by_mechanism: BTreeMap<String, u16>,
    pub dominant_mechanism: String,
    pub entropy_proxy_milli: u16,
    pub coverage_milli: u16,
    pub change_point: bool,
    pub negative_feature_order: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MechanismStateTransitionSupport {
    pub from_timepoint: u16,
    pub to_timepoint: u16,
    pub source_mechanism: String,
    pub destination_mechanism: String,
    pub support_milli: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MechanismStateSmootherDisposition {
    Qualified,
    Partial,
    Unresolved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MechanismStateSmootherResult {
    pub feature_id: String,
    pub output_schema: String,
    pub objective: String,
    pub model_system: GliomaModelSystem,
    pub mechanism_order: Vec<String>,
    pub timepoint_order: Vec<u16>,
    pub posteriors: Vec<MechanismStateSmoothPosterior>,
    pub transition_support: Vec<MechanismStateTransitionSupport>,
    pub dominant_mechanism_order: Vec<String>,
    pub change_point_order: Vec<u16>,
    pub negative_evidence_order: Vec<String>,
    pub uncertainty_order: Vec<String>,
    pub disposition: MechanismStateSmootherDisposition,
    pub next_action: String,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum MechanismStateSmootherError {
    #[error("mechanism state smoother request is invalid: {0}")]
    InvalidRequest(String),
    #[error("mechanism state smoother observation is invalid: {0}")]
    InvalidObservation(String),
    #[error("mechanism state smoother output is invalid: {0}")]
    InvalidOutput(String),
    #[error("mechanism state smoother digest failed: {0}")]
    Digest(String),
}

fn canonical<T: Ord>(values: &[T]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn bounded_value(value: i64) -> bool {
    value.unsigned_abs() <= MAX_ABS_VALUE_MILLI as u64
}

fn digest_input(result: &MechanismStateSmootherResult) -> serde_json::Value {
    serde_json::json!({
        "feature_id": result.feature_id,
        "output_schema": result.output_schema,
        "objective": result.objective,
        "model_system": result.model_system,
        "mechanism_order": result.mechanism_order,
        "timepoint_order": result.timepoint_order,
        "posteriors": result.posteriors,
        "transition_support": result.transition_support,
        "dominant_mechanism_order": result.dominant_mechanism_order,
        "change_point_order": result.change_point_order,
        "negative_evidence_order": result.negative_evidence_order,
        "uncertainty_order": result.uncertainty_order,
        "disposition": result.disposition,
        "next_action": result.next_action,
    })
}

fn validate_request(
    request: &MechanismStateSmootherRequest,
) -> Result<(), MechanismStateSmootherError> {
    if request.objective.trim().is_empty()
        || request.mechanisms.is_empty()
        || request.mechanisms.len() > MAX_MECHANISMS
        || request.observations.is_empty()
        || request.observations.len() > MAX_OBSERVATIONS
        || request.min_coverage_milli > 1_000
        || request.max_entropy_milli > 1_000
        || request.min_transition_support_milli > 1_000
    {
        return Err(MechanismStateSmootherError::InvalidRequest(
            "objective, bounded mechanism/observation sets, and coverage/entropy/transition bounds are required".into(),
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
        return Err(MechanismStateSmootherError::InvalidRequest(
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
            return Err(MechanismStateSmootherError::InvalidObservation(
                "observations require unique positive timepoint/feature keys, bounded values, uncertainty, and valid local de-identified artifacts".into(),
            ));
        }
    }
    Ok(())
}

fn validate_output(
    result: &MechanismStateSmootherResult,
) -> Result<(), MechanismStateSmootherError> {
    if result.feature_id != FEATURE_ID
        || result.output_schema != OUTPUT_SCHEMA
        || result.objective.trim().is_empty()
        || result.mechanism_order.is_empty()
        || !canonical(&result.mechanism_order)
        || result.timepoint_order.is_empty()
        || !canonical(&result.timepoint_order)
        || result.posteriors.len() != result.timepoint_order.len()
        || result.transition_support.len() != result.timepoint_order.len().saturating_sub(1)
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
                || result
                    .mechanism_order
                    .binary_search(&posterior.dominant_mechanism)
                    .is_err()
                || posterior.entropy_proxy_milli > 1_000
                || posterior.coverage_milli > 1_000
                || !canonical(&posterior.negative_feature_order)
        })
        || result.transition_support.windows(2).any(|pair| {
            (pair[0].from_timepoint, pair[0].to_timepoint)
                >= (pair[1].from_timepoint, pair[1].to_timepoint)
        })
        || result.transition_support.iter().any(|support| {
            support.from_timepoint >= support.to_timepoint
                || support.support_milli > 1_000
                || !result
                    .mechanism_order
                    .binary_search(&support.source_mechanism)
                    .is_ok()
                || !result
                    .mechanism_order
                    .binary_search(&support.destination_mechanism)
                    .is_ok()
        })
    {
        return Err(MechanismStateSmootherError::InvalidOutput(
            "identity, ordering, posterior normalization, transition support, or uncertainty invariants are invalid".into(),
        ));
    }
    let expected = ContentHash::of_value(&digest_input(result))
        .map_err(|error| MechanismStateSmootherError::Digest(error.to_string()))?;
    if expected != result.digest {
        return Err(MechanismStateSmootherError::InvalidOutput(
            "mechanism state smoother digest is not content-addressed".into(),
        ));
    }
    Ok(())
}

impl MechanismStateSmootherResult {
    pub fn validate(&self) -> Result<(), MechanismStateSmootherError> {
        validate_output(self)
    }
}

fn compatibility_milli(
    model: &MechanismStateModel,
    observation: &MechanismStateObservation,
) -> Option<u64> {
    let prediction = *model.predictions_milli.get(&observation.feature_id)?;
    let residual = (prediction - observation.observed_milli).unsigned_abs();
    let scale = model
        .process_uncertainty_milli
        .saturating_add(observation.measurement_uncertainty_milli)
        .max(1);
    Some(1_000_u64.saturating_sub((residual.saturating_mul(1_000) / scale).min(1_000)))
}

fn normalize(scores: &[u64]) -> Vec<u16> {
    if scores.is_empty() {
        return Vec::new();
    }
    let total = scores.iter().copied().sum::<u64>();
    if total == 0 {
        let share = 1_000_u16 / scores.len() as u16;
        return scores
            .iter()
            .enumerate()
            .map(|(index, _)| {
                share
                    + if index == 0 {
                        1_000_u16 - share * scores.len() as u16
                    } else {
                        0
                    }
            })
            .collect();
    }
    let mut output = Vec::with_capacity(scores.len());
    let mut assigned = 0_u16;
    for (index, score) in scores.iter().enumerate() {
        let value = if index + 1 == scores.len() {
            1_000_u16.saturating_sub(assigned)
        } else {
            ((*score).saturating_mul(1_000) / total).min(1_000) as u16
        };
        assigned = assigned.saturating_add(value);
        output.push(value);
    }
    output
}

fn entropy_proxy(posterior: &[u16]) -> u16 {
    (1_000_u64
        .saturating_sub(
            posterior
                .iter()
                .map(|value| u64::from(*value) * u64::from(*value) / 1_000)
                .sum::<u64>(),
        )
        .min(1_000)) as u16
}

fn dominant(mechanisms: &[MechanismStateModel], posterior: &[u16]) -> String {
    mechanisms
        .iter()
        .enumerate()
        .max_by(|(left_index, left), (right_index, right)| {
            posterior[*left_index]
                .cmp(&posterior[*right_index])
                .then_with(|| right.mechanism_id.cmp(&left.mechanism_id))
        })
        .map(|(_, mechanism)| mechanism.mechanism_id.clone())
        .unwrap_or_default()
}

fn observation_coverage(
    mechanisms: &[MechanismStateModel],
    observations: &[&MechanismStateObservation],
) -> u16 {
    if observations.is_empty() {
        return 0;
    }
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
}

fn emission(model: &MechanismStateModel, observations: &[&MechanismStateObservation]) -> u64 {
    let compatibilities = observations
        .iter()
        .filter_map(|observation| compatibility_milli(model, observation))
        .collect::<Vec<_>>();
    if compatibilities.is_empty() {
        1_000
    } else {
        compatibilities.iter().sum::<u64>() / compatibilities.len() as u64
    }
}

/// Run a deterministic fixed-interval forward-backward smoother over local mechanism evidence.
pub fn smooth_glioma_mechanism_states(
    request: &MechanismStateSmootherRequest,
) -> Result<MechanismStateSmootherResult, MechanismStateSmootherError> {
    validate_request(request)?;
    let mut mechanisms = request.mechanisms.clone();
    mechanisms.sort_by(|left, right| left.mechanism_id.cmp(&right.mechanism_id));
    let mechanism_order = mechanisms
        .iter()
        .map(|mechanism| mechanism.mechanism_id.clone())
        .collect::<Vec<_>>();
    let timepoint_order = request
        .observations
        .iter()
        .map(|observation| observation.timepoint)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    if timepoint_order.len() > MAX_TIMEPOINTS {
        return Err(MechanismStateSmootherError::InvalidRequest(
            "observation timepoints exceed the bounded smoothing horizon".into(),
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
    let emissions = timepoint_order
        .iter()
        .map(|timepoint| {
            let observations = observations_by_timepoint
                .get(timepoint)
                .cloned()
                .unwrap_or_default();
            mechanisms
                .iter()
                .map(|mechanism| emission(mechanism, &observations))
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    let mechanism_count = mechanisms.len();
    let mut forward = vec![vec![0_u16; mechanism_count]; timepoint_order.len()];
    for time_index in 0..timepoint_order.len() {
        let scores = mechanisms
            .iter()
            .enumerate()
            .map(|(destination, mechanism)| {
                let predicted = if time_index == 0 {
                    u64::from(mechanism.prior_milli)
                } else {
                    mechanisms
                        .iter()
                        .enumerate()
                        .map(|(source, source_model)| {
                            u64::from(forward[time_index - 1][source]).saturating_mul(u64::from(
                                source_model
                                    .transition_milli_by_state
                                    .get(&mechanism.mechanism_id)
                                    .copied()
                                    .unwrap_or(0),
                            )) / 1_000
                        })
                        .sum::<u64>()
                };
                predicted.saturating_mul(emissions[time_index][destination]) / 1_000
            })
            .collect::<Vec<_>>();
        forward[time_index] = normalize(&scores);
    }
    let mut backward = vec![vec![1_000_u16; mechanism_count]; timepoint_order.len()];
    for time_index in (0..timepoint_order.len().saturating_sub(1)).rev() {
        let scores = mechanisms
            .iter()
            .enumerate()
            .map(|(_source, source_model)| {
                mechanisms
                    .iter()
                    .enumerate()
                    .map(|(destination, destination_model)| {
                        u64::from(
                            source_model
                                .transition_milli_by_state
                                .get(&destination_model.mechanism_id)
                                .copied()
                                .unwrap_or(0),
                        )
                        .saturating_mul(emissions[time_index + 1][destination])
                        .saturating_mul(u64::from(backward[time_index + 1][destination]))
                    })
                    .sum::<u64>()
                    / 1_000_000
            })
            .collect::<Vec<_>>();
        backward[time_index] = normalize(&scores);
    }
    let smoothed = forward
        .iter()
        .zip(backward.iter())
        .map(|(left, right)| {
            normalize(
                &left
                    .iter()
                    .zip(right.iter())
                    .map(|(forward, backward)| u64::from(*forward) * u64::from(*backward))
                    .collect::<Vec<_>>(),
            )
        })
        .collect::<Vec<_>>();
    let mut posteriors = Vec::with_capacity(timepoint_order.len());
    let mut dominant_order = Vec::with_capacity(timepoint_order.len());
    let mut negative_evidence = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    for (time_index, timepoint) in timepoint_order.iter().copied().enumerate() {
        let observations = observations_by_timepoint
            .get(&timepoint)
            .cloned()
            .unwrap_or_default();
        let negative_features = observations
            .iter()
            .filter(|observation| {
                mechanisms
                    .iter()
                    .filter_map(|mechanism| compatibility_milli(mechanism, observation))
                    .max()
                    .unwrap_or(0)
                    < 500
            })
            .map(|observation| format!("time-{timepoint}:{}", observation.feature_id))
            .collect::<BTreeSet<_>>();
        let coverage = observation_coverage(&mechanisms, &observations);
        let posterior = &smoothed[time_index];
        let dominant_mechanism = dominant(&mechanisms, posterior);
        let entropy = entropy_proxy(posterior);
        if coverage < request.min_coverage_milli {
            uncertainty.insert(format!("time-{timepoint}:coverage-{coverage}"));
        }
        if entropy > request.max_entropy_milli {
            uncertainty.insert(format!("time-{timepoint}:entropy-{entropy}"));
        }
        negative_evidence.extend(negative_features.iter().cloned());
        dominant_order.push(dominant_mechanism.clone());
        posteriors.push(MechanismStateSmoothPosterior {
            timepoint,
            posterior_milli_by_mechanism: mechanisms
                .iter()
                .enumerate()
                .map(|(index, mechanism)| (mechanism.mechanism_id.clone(), posterior[index]))
                .collect(),
            dominant_mechanism,
            entropy_proxy_milli: entropy,
            coverage_milli: coverage,
            change_point: false,
            negative_feature_order: negative_features.into_iter().collect(),
        });
    }
    let mut transition_support = Vec::with_capacity(timepoint_order.len().saturating_sub(1));
    for time_index in 0..timepoint_order.len().saturating_sub(1) {
        let mut weights = Vec::with_capacity(mechanism_count * mechanism_count);
        for (source, source_model) in mechanisms.iter().enumerate() {
            for (destination, destination_model) in mechanisms.iter().enumerate() {
                weights.push((
                    source,
                    destination,
                    u64::from(smoothed[time_index][source])
                        .saturating_mul(u64::from(
                            source_model
                                .transition_milli_by_state
                                .get(&destination_model.mechanism_id)
                                .copied()
                                .unwrap_or(0),
                        ))
                        .saturating_mul(emissions[time_index + 1][destination])
                        .saturating_mul(u64::from(backward[time_index + 1][destination])),
                ));
            }
        }
        let total = weights.iter().map(|(_, _, weight)| *weight).sum::<u64>();
        let (source, destination, weight) = weights
            .into_iter()
            .max_by(|left, right| {
                left.2
                    .cmp(&right.2)
                    .then_with(|| right.0.cmp(&left.0))
                    .then_with(|| right.1.cmp(&left.1))
            })
            .ok_or_else(|| {
                MechanismStateSmootherError::InvalidOutput(
                    "smoothing produced no transition support".into(),
                )
            })?;
        let support = if total == 0 {
            0
        } else {
            (weight.saturating_mul(1_000) / total).min(1_000) as u16
        };
        let from_timepoint = timepoint_order[time_index];
        let to_timepoint = timepoint_order[time_index + 1];
        if support < request.min_transition_support_milli {
            uncertainty.insert(format!(
                "transition-{from_timepoint}-{to_timepoint}:support-{support}"
            ));
        }
        transition_support.push(MechanismStateTransitionSupport {
            from_timepoint,
            to_timepoint,
            source_mechanism: mechanisms[source].mechanism_id.clone(),
            destination_mechanism: mechanisms[destination].mechanism_id.clone(),
            support_milli: support,
        });
    }
    for index in 0..posteriors.len() {
        let transition_low = index > 0
            && transition_support[index - 1].support_milli < request.min_transition_support_milli;
        let dominant_changed = index > 0
            && posteriors[index - 1].dominant_mechanism != posteriors[index].dominant_mechanism;
        posteriors[index].change_point = dominant_changed
            || posteriors[index].entropy_proxy_milli > request.max_entropy_milli
            || transition_low;
        if posteriors[index].change_point {
            dominant_order[index] = posteriors[index].dominant_mechanism.clone();
        }
    }
    let change_point_order = posteriors
        .iter()
        .filter(|posterior| posterior.change_point)
        .map(|posterior| posterior.timepoint)
        .collect::<Vec<_>>();
    let complete = posteriors.iter().all(|posterior| {
        posterior.coverage_milli >= request.min_coverage_milli
            && posterior.entropy_proxy_milli <= request.max_entropy_milli
    }) && transition_support
        .iter()
        .all(|support| support.support_milli >= request.min_transition_support_milli);
    let disposition = if posteriors.is_empty() {
        MechanismStateSmootherDisposition::Unresolved
    } else if complete {
        MechanismStateSmootherDisposition::Qualified
    } else {
        MechanismStateSmootherDisposition::Partial
    };
    let next_action = match disposition {
        MechanismStateSmootherDisposition::Qualified => {
            "send the smoothed trajectory and supported transitions to counterfactual and experiment-design planning"
        }
        MechanismStateSmootherDisposition::Partial => {
            "acquire missing or transition-discriminating local features before promoting the retrospective trajectory"
        }
        MechanismStateSmootherDisposition::Unresolved => {
            "provide ordered local observations and typed mechanism transitions before smoothing"
        }
    }
    .to_string();
    let mut result = MechanismStateSmootherResult {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        objective: request.objective.clone(),
        model_system: request.model_system,
        mechanism_order,
        timepoint_order,
        posteriors,
        transition_support,
        dominant_mechanism_order: dominant_order,
        change_point_order,
        negative_evidence_order: negative_evidence.into_iter().collect(),
        uncertainty_order: uncertainty.into_iter().collect(),
        disposition,
        next_action,
        digest: ContentHash::of_bytes(b"unsealed-glioma-mechanism-state-smoother"),
    };
    result.digest = ContentHash::of_value(&digest_input(&result))
        .map_err(|error| MechanismStateSmootherError::Digest(error.to_string()))?;
    validate_output(&result)?;
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::glioma_engine::{GliomaModality, LocalArtifactRef};

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

    fn model(
        id: &str,
        prior: u16,
        self_transition: u16,
        other: u16,
        prediction: i64,
    ) -> MechanismStateModel {
        let other_id = if id == "growth" { "stress" } else { "growth" };
        MechanismStateModel {
            mechanism_id: id.into(),
            statement: format!("{id} mechanism"),
            prior_milli: prior,
            transition_milli_by_state: BTreeMap::from([
                (id.into(), self_transition),
                (other_id.into(), other),
            ]),
            predictions_milli: BTreeMap::from([("invasion-score".into(), prediction)]),
            process_uncertainty_milli: 100,
        }
    }

    fn observation(timepoint: u16, value: i64, id: &str) -> MechanismStateObservation {
        MechanismStateObservation {
            timepoint,
            feature_id: "invasion-score".into(),
            modality: GliomaModality::Imaging,
            observed_milli: value,
            measurement_uncertainty_milli: 50,
            artifact: artifact(id),
        }
    }

    fn request(observations: Vec<MechanismStateObservation>) -> MechanismStateSmootherRequest {
        MechanismStateSmootherRequest {
            objective: "smooth longitudinal invasion mechanisms".into(),
            model_system: GliomaModelSystem::Organoid,
            mechanisms: vec![
                model("growth", 500, 900, 100, 800),
                model("stress", 500, 900, 100, 200),
            ],
            observations,
            min_coverage_milli: 800,
            max_entropy_milli: 950,
            min_transition_support_milli: 0,
        }
    }

    #[test]
    fn smoother_uses_future_observations_and_normalizes_transitions() {
        let result = smooth_glioma_mechanism_states(&request(vec![
            observation(1, 790, "a"),
            observation(2, 210, "b"),
        ]))
        .unwrap();
        result.validate().unwrap();
        assert_eq!(result.posteriors.len(), 2);
        assert_eq!(result.transition_support.len(), 1);
        assert_eq!(
            result.posteriors[0]
                .posterior_milli_by_mechanism
                .values()
                .sum::<u16>(),
            1_000
        );
        assert!(result.transition_support[0].support_milli > 0);
    }

    #[test]
    fn unknown_features_are_explicitly_partial() {
        let mut request = request(vec![observation(1, 500, "unknown")]);
        request.observations[0].feature_id = "unmodeled-feature".into();
        request.min_coverage_milli = 1_000;
        let result = smooth_glioma_mechanism_states(&request).unwrap();
        assert_eq!(
            result.disposition,
            MechanismStateSmootherDisposition::Partial
        );
        assert!(result
            .uncertainty_order
            .iter()
            .any(|item| item.contains("coverage")));
    }

    #[test]
    fn contradictory_observation_is_negative_and_replay_stable() {
        let request = request(vec![observation(1, 1_000, "a"), observation(2, 1_000, "b")]);
        let first = smooth_glioma_mechanism_states(&request).unwrap();
        let replay = smooth_glioma_mechanism_states(&request).unwrap();
        assert_eq!(first, replay);
        assert!(!first.negative_evidence_order.is_empty());
        assert!(first
            .posteriors
            .iter()
            .any(|posterior| !posterior.negative_feature_order.is_empty()));
    }
}
