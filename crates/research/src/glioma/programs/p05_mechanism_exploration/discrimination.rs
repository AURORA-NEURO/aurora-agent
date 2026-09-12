//! Mechanism discrimination and next-assay information gain for preclinical glioma research.
//!
//! This product compares competing mechanistic predictions against local, de-identified feature
//! observations and ranks the next assay by expected separation of the current mechanism
//! posterior.  It is deliberately not a causal oracle: prediction residuals, missing features,
//! diffuse posteriors, and an information-poor action set remain explicit in the output.

use crate::glioma_engine::{GliomaModelSystem, LocalArtifactRef};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P05-F09";
pub const OUTPUT_SCHEMA: &str = "GliomaMechanismDiscrimination1@1";
pub const MAX_MECHANISMS: usize = 256;
pub const MAX_PREDICTIONS_PER_MECHANISM: usize = 4_096;
pub const MAX_OBSERVATIONS: usize = 16_384;
pub const MAX_ACTIONS: usize = 4_096;
pub const MAX_VALUE_MILLI: u64 = 1_000_000;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MechanismDiscriminationRequest {
    pub objective: String,
    pub model_system: GliomaModelSystem,
    pub min_shared_features: usize,
    pub max_mechanisms: usize,
    pub max_actions: usize,
    pub min_information_gain_milli: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MechanismPrediction {
    pub feature_id: String,
    pub predicted_milli: i64,
    pub uncertainty_milli: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MechanismHypothesis {
    pub mechanism_id: String,
    pub statement: String,
    pub predictions: Vec<MechanismPrediction>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MechanismFeatureObservation {
    pub feature_id: String,
    pub observed_milli: i64,
    pub uncertainty_milli: u64,
    pub artifact: LocalArtifactRef,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MechanismDiscriminatorAction {
    pub action_id: String,
    pub feature_id: String,
    pub predicted_milli_by_mechanism: BTreeMap<String, i64>,
    pub measurement_uncertainty_milli: u64,
    pub feasibility_milli: u16,
    pub cost_units: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MechanismDiscriminationRanking {
    pub mechanism_id: String,
    pub matched_feature_order: Vec<String>,
    pub missing_feature_order: Vec<String>,
    pub residual_loss_milli: u64,
    pub coverage_milli: u16,
    pub fit_score_milli: u64,
    pub posterior_milli: u16,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MechanismInformationGain {
    pub action_id: String,
    pub feature_id: String,
    pub mechanism_order: Vec<String>,
    pub expected_information_milli: u64,
    pub adjusted_information_milli: u64,
    pub measurement_uncertainty_milli: u64,
    pub feasibility_milli: u16,
    pub cost_units: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MechanismDiscriminationDisposition {
    Qualified,
    Partial,
    Unresolved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MechanismDiscrimination {
    pub feature_id: String,
    pub output_schema: String,
    pub objective: String,
    pub model_system: GliomaModelSystem,
    pub mechanism_order: Vec<String>,
    pub rankings: Vec<MechanismDiscriminationRanking>,
    pub action_order: Vec<String>,
    pub actions: Vec<MechanismInformationGain>,
    pub selected_action_order: Vec<String>,
    pub unresolved_mechanism_order: Vec<String>,
    pub negative_evidence: Vec<String>,
    pub uncertainty: Vec<String>,
    pub disposition: MechanismDiscriminationDisposition,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum MechanismDiscriminationError {
    #[error("mechanism discrimination request is invalid: {0}")]
    InvalidRequest(String),
    #[error("mechanism discrimination input is invalid: {0}")]
    InvalidInput(String),
    #[error("mechanism discrimination output is invalid: {0}")]
    InvalidOutput(String),
    #[error("mechanism discrimination digest failed: {0}")]
    Digest(String),
}

fn digest_input(output: &MechanismDiscrimination) -> serde_json::Value {
    serde_json::json!({
        "feature_id": output.feature_id,
        "output_schema": output.output_schema,
        "objective": output.objective,
        "model_system": output.model_system,
        "mechanism_order": output.mechanism_order,
        "rankings": output.rankings,
        "action_order": output.action_order,
        "actions": output.actions,
        "selected_action_order": output.selected_action_order,
        "unresolved_mechanism_order": output.unresolved_mechanism_order,
        "negative_evidence": output.negative_evidence,
        "uncertainty": output.uncertainty,
        "disposition": output.disposition,
    })
}

impl MechanismDiscrimination {
    pub fn validate(&self) -> Result<(), MechanismDiscriminationError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || self.objective.trim().is_empty()
            || self
                .mechanism_order
                .windows(2)
                .any(|pair| pair[0] >= pair[1])
            || self.rankings.windows(2).any(|pair| {
                pair[0].fit_score_milli < pair[1].fit_score_milli
                    || (pair[0].fit_score_milli == pair[1].fit_score_milli
                        && pair[0].mechanism_id > pair[1].mechanism_id)
            })
            || self
                .rankings
                .iter()
                .any(|item| item.coverage_milli > 1_000 || item.fit_score_milli > 1_000_000)
            || self.action_order.windows(2).any(|pair| pair[0] >= pair[1])
            || self.actions.windows(2).any(|pair| {
                pair[0].adjusted_information_milli < pair[1].adjusted_information_milli
                    || (pair[0].adjusted_information_milli == pair[1].adjusted_information_milli
                        && pair[0].action_id > pair[1].action_id)
            })
            || self.actions.iter().any(|item| {
                item.action_id.trim().is_empty()
                    || item.feature_id.trim().is_empty()
                    || item.measurement_uncertainty_milli == 0
                    || item.feasibility_milli > 1_000
                    || item.cost_units == 0
                    || item
                        .mechanism_order
                        .windows(2)
                        .any(|pair| pair[0] >= pair[1])
            })
            || self
                .unresolved_mechanism_order
                .windows(2)
                .any(|pair| pair[0] >= pair[1])
            || self
                .negative_evidence
                .windows(2)
                .any(|pair| pair[0] >= pair[1])
            || self.uncertainty.windows(2).any(|pair| pair[0] >= pair[1])
        {
            return Err(MechanismDiscriminationError::InvalidOutput(
                "identity, bounds, ranking, or canonical ordering is invalid".into(),
            ));
        }
        let mechanism_ids = self
            .mechanism_order
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>();
        let ranking_ids = self
            .rankings
            .iter()
            .map(|item| item.mechanism_id.clone())
            .collect::<BTreeSet<_>>();
        let action_ids = self.action_order.iter().cloned().collect::<BTreeSet<_>>();
        let output_action_ids = self
            .actions
            .iter()
            .map(|item| item.action_id.clone())
            .collect::<BTreeSet<_>>();
        let selected_action_ids = self
            .selected_action_order
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>();
        let posterior_sum = self
            .rankings
            .iter()
            .map(|item| u32::from(item.posterior_milli))
            .sum::<u32>();
        if mechanism_ids != ranking_ids
            || action_ids != output_action_ids
            || selected_action_ids.len() != self.selected_action_order.len()
            || self
                .selected_action_order
                .iter()
                .any(|id| !action_ids.contains(id))
            || !self
                .actions
                .iter()
                .take(self.selected_action_order.len())
                .map(|item| item.action_id.as_str())
                .eq(self.selected_action_order.iter().map(String::as_str))
            || posterior_sum != 1_000
            || self.rankings.iter().any(|item| {
                item.matched_feature_order
                    .windows(2)
                    .any(|pair| pair[0] >= pair[1])
            })
        {
            return Err(MechanismDiscriminationError::InvalidOutput(
                "mechanism/action partitions do not reconcile".into(),
            ));
        }
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| MechanismDiscriminationError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(MechanismDiscriminationError::InvalidOutput(
                "digest is not bound to the mechanism discrimination".into(),
            ));
        }
        Ok(())
    }
}

fn score_for_loss(loss_milli: u64) -> u64 {
    1_000_000_u64
        .checked_div(1 + loss_milli / 1_000)
        .unwrap_or(0)
}

fn expected_information(
    action: &MechanismDiscriminatorAction,
    posterior: &BTreeMap<String, u16>,
) -> u64 {
    let ids = posterior.keys().cloned().collect::<Vec<_>>();
    let mut information = 0_u128;
    for (index, left) in ids.iter().enumerate() {
        for right in ids.iter().skip(index + 1) {
            let Some(left_prediction) = action.predicted_milli_by_mechanism.get(left) else {
                continue;
            };
            let Some(right_prediction) = action.predicted_milli_by_mechanism.get(right) else {
                continue;
            };
            let difference = (*left_prediction as i128 - *right_prediction as i128).unsigned_abs();
            let pair_probability = u128::from(posterior[left]) * u128::from(posterior[right]);
            let denominator = u128::from(action.measurement_uncertainty_milli)
                .saturating_mul(u128::from(action.measurement_uncertainty_milli))
                .max(1);
            information = information.saturating_add(
                pair_probability
                    .saturating_mul(difference.saturating_mul(difference))
                    .saturating_mul(1_000)
                    / denominator,
            );
        }
    }
    information
        .checked_div(1_000_000)
        .unwrap_or(0)
        .min(u128::from(u64::MAX)) as u64
}

pub fn discriminate_mechanisms(
    request: &MechanismDiscriminationRequest,
    hypotheses: &[MechanismHypothesis],
    observations: &[MechanismFeatureObservation],
    actions: &[MechanismDiscriminatorAction],
) -> Result<MechanismDiscrimination, MechanismDiscriminationError> {
    if request.objective.trim().is_empty()
        || request.min_shared_features == 0
        || request.max_mechanisms == 0
        || request.max_actions == 0
        || request.min_information_gain_milli == 0
        || hypotheses.is_empty()
        || hypotheses.len() > request.max_mechanisms
        || hypotheses.len() > MAX_MECHANISMS
        || observations.len() > MAX_OBSERVATIONS
        || actions.len() > MAX_ACTIONS
    {
        return Err(MechanismDiscriminationError::InvalidRequest(
            "objective, mechanism/feature/action floors, bounds, and a non-empty hypothesis set are required".into(),
        ));
    }
    let mut mechanism_ids = BTreeSet::new();
    let mut observation_map = BTreeMap::<String, &MechanismFeatureObservation>::new();
    for observation in observations {
        observation
            .artifact
            .validate()
            .map_err(|error| MechanismDiscriminationError::InvalidInput(error.to_string()))?;
        if observation.feature_id.trim().is_empty()
            || observation.uncertainty_milli == 0
            || observation.uncertainty_milli > MAX_VALUE_MILLI
            || observation.observed_milli.unsigned_abs() > MAX_VALUE_MILLI
            || observation_map
                .insert(observation.feature_id.clone(), observation)
                .is_some()
        {
            return Err(MechanismDiscriminationError::InvalidInput(
                "observation identity, bounds, and uniqueness are invalid".into(),
            ));
        }
    }
    let mut hypothesis_features = BTreeMap::<String, BTreeSet<String>>::new();
    for hypothesis in hypotheses {
        if hypothesis.mechanism_id.trim().is_empty()
            || hypothesis.statement.trim().is_empty()
            || hypothesis.predictions.is_empty()
            || hypothesis.predictions.len() > MAX_PREDICTIONS_PER_MECHANISM
            || hypothesis
                .predictions
                .windows(2)
                .any(|pair| pair[0].feature_id >= pair[1].feature_id)
            || !mechanism_ids.insert(hypothesis.mechanism_id.clone())
        {
            return Err(MechanismDiscriminationError::InvalidInput(
                "hypothesis identity, statement, prediction bound, or uniqueness is invalid".into(),
            ));
        }
        let mut features = BTreeSet::new();
        for prediction in &hypothesis.predictions {
            if prediction.feature_id.trim().is_empty()
                || prediction.uncertainty_milli == 0
                || prediction.uncertainty_milli > MAX_VALUE_MILLI
                || prediction.predicted_milli.unsigned_abs() > MAX_VALUE_MILLI
                || !features.insert(prediction.feature_id.clone())
            {
                return Err(MechanismDiscriminationError::InvalidInput(
                    "prediction identity, bounds, or uniqueness are invalid".into(),
                ));
            }
        }
        hypothesis_features.insert(hypothesis.mechanism_id.clone(), features);
    }
    let mut action_ids = BTreeSet::new();
    for action in actions {
        if action.action_id.trim().is_empty()
            || action.feature_id.trim().is_empty()
            || action.measurement_uncertainty_milli == 0
            || action.measurement_uncertainty_milli > MAX_VALUE_MILLI
            || action.feasibility_milli > 1_000
            || action.cost_units == 0
            || action.predicted_milli_by_mechanism.len() != mechanism_ids.len()
            || action
                .predicted_milli_by_mechanism
                .keys()
                .any(|id| !mechanism_ids.contains(id))
            || action
                .predicted_milli_by_mechanism
                .values()
                .any(|value| value.unsigned_abs() > MAX_VALUE_MILLI)
            || !action_ids.insert(action.action_id.clone())
        {
            return Err(MechanismDiscriminationError::InvalidInput(
                "action identity, prediction coverage, bounds, cost, feasibility, or uniqueness is invalid".into(),
            ));
        }
    }
    let mut preliminary = Vec::with_capacity(hypotheses.len());
    let mut unresolved_mechanisms = BTreeSet::new();
    for hypothesis in hypotheses {
        let mut matched = Vec::new();
        let mut missing = Vec::new();
        let mut loss = 0_u64;
        for prediction in &hypothesis.predictions {
            match observation_map.get(&prediction.feature_id) {
                Some(observation) => {
                    matched.push(prediction.feature_id.clone());
                    let residual =
                        (prediction.predicted_milli - observation.observed_milli).unsigned_abs();
                    let uncertainty = prediction
                        .uncertainty_milli
                        .saturating_add(observation.uncertainty_milli);
                    let denominator = u128::from(uncertainty)
                        .saturating_mul(u128::from(uncertainty))
                        .max(1);
                    let standardized = u128::from(residual)
                        .saturating_mul(u128::from(residual))
                        .saturating_mul(1_000)
                        .checked_div(denominator)
                        .unwrap_or(0)
                        .min(u128::from(u64::MAX)) as u64;
                    loss = loss.saturating_add(standardized);
                }
                None => missing.push(prediction.feature_id.clone()),
            }
        }
        let coverage = ((matched.len() * 1_000) / hypothesis.predictions.len()) as u16;
        if matched.len() < request.min_shared_features {
            unresolved_mechanisms.insert(hypothesis.mechanism_id.clone());
        }
        let fit = score_for_loss(loss)
            .saturating_mul(u64::from(coverage))
            .checked_div(1_000)
            .unwrap_or(0);
        preliminary.push((
            hypothesis.mechanism_id.clone(),
            matched,
            missing,
            loss,
            coverage,
            fit,
        ));
    }
    preliminary.sort_by(|left, right| right.5.cmp(&left.5).then_with(|| left.0.cmp(&right.0)));
    let total_fit = preliminary.iter().map(|item| item.5).sum::<u64>().max(1);
    let mut posterior = BTreeMap::new();
    for item in &preliminary {
        posterior.insert(
            item.0.clone(),
            (item.5.saturating_mul(1_000) / total_fit).min(1_000) as u16,
        );
    }
    let assigned = posterior
        .values()
        .map(|value| u32::from(*value))
        .sum::<u32>();
    if let Some(top) = preliminary.first() {
        let remainder = 1_000_u32.saturating_sub(assigned);
        if remainder > 0 {
            posterior.insert(
                top.0.clone(),
                posterior[&top.0].saturating_add(remainder.min(1_000) as u16),
            );
        }
    }
    let rankings = preliminary
        .iter()
        .map(|item| MechanismDiscriminationRanking {
            mechanism_id: item.0.clone(),
            matched_feature_order: item.1.clone(),
            missing_feature_order: item.2.clone(),
            residual_loss_milli: item.3,
            coverage_milli: item.4,
            fit_score_milli: item.5,
            posterior_milli: posterior[&item.0],
        })
        .collect::<Vec<_>>();
    let mechanism_order = mechanism_ids.iter().cloned().collect::<Vec<_>>();
    let mut information = actions
        .iter()
        .map(|action| {
            let expected = expected_information(action, &posterior);
            let adjusted = expected
                .saturating_mul(u64::from(action.feasibility_milli))
                .checked_div(1_000)
                .unwrap_or(0)
                .checked_div(u64::from(action.cost_units))
                .unwrap_or(0);
            MechanismInformationGain {
                action_id: action.action_id.clone(),
                feature_id: action.feature_id.clone(),
                mechanism_order: mechanism_order.clone(),
                expected_information_milli: expected,
                adjusted_information_milli: adjusted,
                measurement_uncertainty_milli: action.measurement_uncertainty_milli,
                feasibility_milli: action.feasibility_milli,
                cost_units: action.cost_units,
            }
        })
        .collect::<Vec<_>>();
    information.sort_by(|left, right| {
        right
            .adjusted_information_milli
            .cmp(&left.adjusted_information_milli)
            .then_with(|| left.action_id.cmp(&right.action_id))
    });
    let action_order = actions
        .iter()
        .map(|action| action.action_id.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let selected_action_order = information
        .iter()
        .filter(|item| item.adjusted_information_milli >= request.min_information_gain_milli)
        .take(request.max_actions)
        .map(|item| item.action_id.clone())
        .collect::<Vec<_>>();
    let mut negative = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    if observations.is_empty() {
        negative.insert("no-local-feature-observations-provided".into());
    }
    if preliminary.iter().all(|item| item.5 == 0) {
        negative.insert("no-mechanism-fits-observed-evidence".into());
    }
    if selected_action_order.is_empty() {
        negative.insert("no-discriminating-action-clears-information-floor".into());
    }
    if !unresolved_mechanisms.is_empty() {
        uncertainty.insert("one-or-more-mechanisms-below-shared-feature-floor".into());
    }
    if rankings.len() > 1
        && rankings[0]
            .posterior_milli
            .saturating_sub(rankings[1].posterior_milli)
            < 100
    {
        uncertainty.insert("mechanism-posterior-remains-diffuse".into());
    }
    let disposition = if observations.is_empty() || unresolved_mechanisms.len() == hypotheses.len()
    {
        MechanismDiscriminationDisposition::Unresolved
    } else if !unresolved_mechanisms.is_empty() || selected_action_order.is_empty() {
        MechanismDiscriminationDisposition::Partial
    } else {
        MechanismDiscriminationDisposition::Qualified
    };
    let mut output = MechanismDiscrimination {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        objective: request.objective.clone(),
        model_system: request.model_system,
        mechanism_order,
        rankings,
        action_order,
        actions: information,
        selected_action_order,
        unresolved_mechanism_order: unresolved_mechanisms.into_iter().collect(),
        negative_evidence: negative.into_iter().collect(),
        uncertainty: uncertainty.into_iter().collect(),
        disposition,
        digest: ContentHash::of_value(&serde_json::json!({}))
            .map_err(|error| MechanismDiscriminationError::Digest(error.to_string()))?,
    };
    output.digest = ContentHash::of_value(&digest_input(&output))
        .map_err(|error| MechanismDiscriminationError::Digest(error.to_string()))?;
    output.validate()?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hash(id: &str) -> ContentHash {
        ContentHash::of_value(&serde_json::json!({"id": id})).unwrap()
    }

    fn observation(feature_id: &str, value: i64) -> MechanismFeatureObservation {
        MechanismFeatureObservation {
            feature_id: feature_id.into(),
            observed_milli: value,
            uncertainty_milli: 10,
            artifact: LocalArtifactRef {
                artifact_id: "local-observation".into(),
                content_hash: hash(feature_id),
                content_type: "application/vnd.aurora.glioma-feature+json".into(),
                local_only: true,
                contains_human_data: false,
                contains_direct_identifiers: false,
            },
        }
    }

    fn request() -> MechanismDiscriminationRequest {
        MechanismDiscriminationRequest {
            objective: "discriminate invasion mechanisms".into(),
            model_system: GliomaModelSystem::Organoid,
            min_shared_features: 2,
            max_mechanisms: 4,
            max_actions: 4,
            min_information_gain_milli: 10,
        }
    }

    #[test]
    fn exact_mechanism_fit_selects_informative_action() {
        let hypotheses = vec![
            MechanismHypothesis {
                mechanism_id: "motility".into(),
                statement: "motility pathway drives invasion".into(),
                predictions: vec![
                    MechanismPrediction {
                        feature_id: "f1".into(),
                        predicted_milli: 100,
                        uncertainty_milli: 10,
                    },
                    MechanismPrediction {
                        feature_id: "f2".into(),
                        predicted_milli: 200,
                        uncertainty_milli: 10,
                    },
                ],
            },
            MechanismHypothesis {
                mechanism_id: "matrix".into(),
                statement: "matrix remodeling drives invasion".into(),
                predictions: vec![
                    MechanismPrediction {
                        feature_id: "f1".into(),
                        predicted_milli: 400,
                        uncertainty_milli: 10,
                    },
                    MechanismPrediction {
                        feature_id: "f2".into(),
                        predicted_milli: 500,
                        uncertainty_milli: 10,
                    },
                ],
            },
        ];
        let actions = vec![MechanismDiscriminatorAction {
            action_id: "perturb-f1".into(),
            feature_id: "f1".into(),
            predicted_milli_by_mechanism: BTreeMap::from([
                ("matrix".into(), 500),
                ("motility".into(), 100),
            ]),
            measurement_uncertainty_milli: 20,
            feasibility_milli: 1_000,
            cost_units: 1,
        }];
        let output = discriminate_mechanisms(
            &request(),
            &hypotheses,
            &[observation("f1", 100), observation("f2", 200)],
            &actions,
        )
        .unwrap();
        assert_eq!(
            output.disposition,
            MechanismDiscriminationDisposition::Qualified
        );
        assert_eq!(output.rankings[0].mechanism_id, "motility");
        assert_eq!(output.selected_action_order, vec!["perturb-f1"]);
        output.validate().unwrap();
    }

    #[test]
    fn missing_features_remain_partial_and_replay_stable() {
        let hypotheses = vec![MechanismHypothesis {
            mechanism_id: "motility".into(),
            statement: "motility pathway drives invasion".into(),
            predictions: vec![
                MechanismPrediction {
                    feature_id: "f1".into(),
                    predicted_milli: 100,
                    uncertainty_milli: 10,
                },
                MechanismPrediction {
                    feature_id: "f2".into(),
                    predicted_milli: 200,
                    uncertainty_milli: 10,
                },
            ],
        }];
        let first =
            discriminate_mechanisms(&request(), &hypotheses, &[observation("f1", 100)], &[])
                .unwrap();
        let second =
            discriminate_mechanisms(&request(), &hypotheses, &[observation("f1", 100)], &[])
                .unwrap();
        assert_eq!(first, second);
        assert_eq!(
            first.disposition,
            MechanismDiscriminationDisposition::Unresolved
        );
        assert_eq!(first.unresolved_mechanism_order, vec!["motility"]);
    }
}
