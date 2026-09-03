//! Robust, model-ensemble active learning for preclinical glioma assays.
//!
//! This planner is deliberately different from the single-surrogate active learner: it keeps
//! competing mechanistic surrogates alive, computes a conservative lower-tail acquisition under
//! model disagreement, and refuses to turn contradictory local observations into confidence. It
//! compiles a safe next batch only; a caller-owned laboratory or computation gateway performs any
//! physical or external effect.

use super::active_learning::ActiveLearningDirection;
use crate::glioma_engine::{GliomaModelSystem, LocalArtifactRef};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P06-F13";
pub const OUTPUT_SCHEMA: &str = "GliomaRobustActiveLearning1@1";
pub const MAX_CANDIDATES: usize = 2_048;
pub const MAX_MODELS: usize = 128;
pub const MAX_OBSERVATIONS: usize = 32_768;
pub const MAX_FEATURES: usize = 512;
const SCORE_SCALE: i128 = 1_000;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RobustActiveLearningModel {
    pub model_id: String,
    pub prior_weight_milli: u32,
    pub intercept_milli: i64,
    pub feature_weights: Vec<i64>,
    pub residual_milli: u16,
    pub reliability_milli: u16,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RobustActiveLearningCandidate {
    pub candidate_id: String,
    pub mechanism_id: String,
    pub feature_vector: Vec<i64>,
    pub cost_units: u32,
    pub risk_milli: u16,
    pub max_replicates: u16,
    pub redundancy_group: String,
    pub output_schema: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RobustActiveLearningObservation {
    pub observation_id: String,
    pub candidate_id: String,
    pub outcome_milli: i64,
    pub uncertainty_milli: u16,
    pub artifact: LocalArtifactRef,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RobustActiveLearningRequest {
    pub objective: String,
    pub model_system: GliomaModelSystem,
    pub direction: ActiveLearningDirection,
    pub budget_units: u32,
    pub max_selections: usize,
    pub min_observations_per_candidate: usize,
    pub lower_tail_weight_milli: u16,
    pub disagreement_weight_milli: u16,
    pub information_weight_milli: u16,
    pub cost_penalty_milli: u16,
    pub risk_penalty_milli: u16,
    pub max_risk_milli: u16,
    pub min_model_reliability_milli: u16,
    pub models: Vec<RobustActiveLearningModel>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RobustActiveLearningCandidateDisposition {
    Selected,
    Deferred,
    Blocked,
    Unresolved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RobustActiveLearningScore {
    pub candidate_id: String,
    pub weighted_mean_milli: i64,
    pub lower_tail_milli: i64,
    pub model_disagreement_milli: u32,
    pub posterior_uncertainty_milli: u16,
    pub expected_information_milli: u16,
    pub model_support_count: u16,
    pub direct_observation_count: u16,
    pub acquisition_milli: i64,
    pub disposition: RobustActiveLearningCandidateDisposition,
    pub rationale: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RobustActiveLearningDisposition {
    Qualified,
    Partial,
    NoCandidates,
    Unresolved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RobustActiveLearningPlan {
    pub feature_id: String,
    pub output_schema: String,
    pub objective: String,
    pub model_system: GliomaModelSystem,
    pub candidate_order: Vec<String>,
    pub observation_order: Vec<String>,
    pub scores: Vec<RobustActiveLearningScore>,
    pub selected_order: Vec<String>,
    pub deferred_order: Vec<String>,
    pub blocked_order: Vec<String>,
    pub unresolved_order: Vec<String>,
    pub remaining_budget_units: u32,
    pub negative_evidence: Vec<String>,
    pub uncertainty: Vec<String>,
    pub disposition: RobustActiveLearningDisposition,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum RobustActiveLearningError {
    #[error("robust active-learning request is invalid: {0}")]
    InvalidRequest(String),
    #[error("robust active-learning observation is invalid: {0}")]
    InvalidObservation(String),
    #[error("robust active-learning output is invalid: {0}")]
    InvalidOutput(String),
    #[error("robust active-learning digest failed: {0}")]
    Digest(String),
}

fn canonical(values: &[String]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn signed(direction: ActiveLearningDirection, value: i64) -> i64 {
    match direction {
        ActiveLearningDirection::Maximize => value,
        ActiveLearningDirection::Minimize => value.saturating_neg(),
    }
}

fn digest_input(plan: &RobustActiveLearningPlan) -> serde_json::Value {
    serde_json::json!({
        "feature_id": plan.feature_id,
        "output_schema": plan.output_schema,
        "objective": plan.objective,
        "model_system": plan.model_system,
        "candidate_order": plan.candidate_order,
        "observation_order": plan.observation_order,
        "scores": plan.scores,
        "selected_order": plan.selected_order,
        "deferred_order": plan.deferred_order,
        "blocked_order": plan.blocked_order,
        "unresolved_order": plan.unresolved_order,
        "remaining_budget_units": plan.remaining_budget_units,
        "negative_evidence": plan.negative_evidence,
        "uncertainty": plan.uncertainty,
        "disposition": plan.disposition,
    })
}

impl RobustActiveLearningPlan {
    pub fn validate(&self) -> Result<(), RobustActiveLearningError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || self.objective.trim().is_empty()
            || self
                .candidate_order
                .windows(2)
                .any(|pair| pair[0] >= pair[1])
            || !canonical(&self.observation_order)
            || !canonical(&self.deferred_order)
            || !canonical(&self.blocked_order)
            || !canonical(&self.unresolved_order)
            || !canonical(&self.negative_evidence)
            || !canonical(&self.uncertainty)
            || self.scores.len() != self.candidate_order.len()
            || self.scores.iter().any(|score| {
                score.candidate_id.trim().is_empty()
                    || score.rationale.trim().is_empty()
                    || score.posterior_uncertainty_milli > 1_000
                    || score.expected_information_milli > 1_000
            })
        {
            return Err(RobustActiveLearningError::InvalidOutput(
                "identity, bounds, score cardinality, or canonical ordering is invalid".into(),
            ));
        }
        let candidates = self
            .candidate_order
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>();
        let scores = self
            .scores
            .iter()
            .map(|score| score.candidate_id.clone())
            .collect::<BTreeSet<_>>();
        if candidates != scores {
            return Err(RobustActiveLearningError::InvalidOutput(
                "candidate and score identities do not reconcile".into(),
            ));
        }
        let selected = self.selected_order.iter().cloned().collect::<BTreeSet<_>>();
        let deferred = self.deferred_order.iter().cloned().collect::<BTreeSet<_>>();
        let blocked = self.blocked_order.iter().cloned().collect::<BTreeSet<_>>();
        let unresolved = self
            .unresolved_order
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>();
        if selected.len() != self.selected_order.len()
            || deferred.len() != self.deferred_order.len()
            || selected.intersection(&deferred).next().is_some()
            || selected.intersection(&blocked).next().is_some()
            || selected.intersection(&unresolved).next().is_some()
            || deferred.intersection(&blocked).next().is_some()
            || deferred.intersection(&unresolved).next().is_some()
            || blocked.intersection(&unresolved).next().is_some()
            || selected
                .union(&deferred)
                .cloned()
                .chain(blocked.iter().cloned())
                .chain(unresolved.iter().cloned())
                .collect::<BTreeSet<_>>()
                != candidates
        {
            return Err(RobustActiveLearningError::InvalidOutput(
                "candidate disposition partitions do not reconcile".into(),
            ));
        }
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| RobustActiveLearningError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(RobustActiveLearningError::InvalidOutput(
                "robust active-learning digest is not content-addressed".into(),
            ));
        }
        Ok(())
    }
}

fn validate_request(
    request: &RobustActiveLearningRequest,
    candidates: &[RobustActiveLearningCandidate],
    observations: &[RobustActiveLearningObservation],
) -> Result<(), RobustActiveLearningError> {
    if request.objective.trim().is_empty()
        || request.budget_units == 0
        || request.max_selections == 0
        || request.max_selections > MAX_CANDIDATES
        || request.min_observations_per_candidate == 0
        || request.models.is_empty()
        || request.models.len() > MAX_MODELS
        || candidates.len() > MAX_CANDIDATES
        || observations.len() > MAX_OBSERVATIONS
        || request.models.iter().any(|model| {
            model.model_id.trim().is_empty()
                || model.prior_weight_milli == 0
                || model.feature_weights.is_empty()
                || model.feature_weights.len() > MAX_FEATURES
                || model.residual_milli > 1_000
                || model.reliability_milli > 1_000
        })
    {
        return Err(RobustActiveLearningError::InvalidRequest(
            "objective, bounded budget/selections, models, priors, and reliability bounds are required".into(),
        ));
    }
    let mut model_ids = BTreeSet::new();
    let dimensions = request.models[0].feature_weights.len();
    if request.models.iter().any(|model| {
        model.feature_weights.len() != dimensions || !model_ids.insert(model.model_id.clone())
    }) {
        return Err(RobustActiveLearningError::InvalidRequest(
            "model ids and feature dimensions must be unique and aligned".into(),
        ));
    }
    let mut candidate_ids = BTreeSet::new();
    if candidates.iter().any(|candidate| {
        candidate.candidate_id.trim().is_empty()
            || candidate.mechanism_id.trim().is_empty()
            || candidate.feature_vector.len() != dimensions
            || candidate.feature_vector.len() > MAX_FEATURES
            || candidate.cost_units == 0
            || candidate.risk_milli > 1_000
            || candidate.max_replicates == 0
            || candidate.redundancy_group.trim().is_empty()
            || candidate.output_schema.trim().is_empty()
            || !candidate_ids.insert(candidate.candidate_id.clone())
    }) {
        return Err(RobustActiveLearningError::InvalidRequest(
            "candidate ids, dimensions, costs, risk, replicate ceilings, and contracts are required".into(),
        ));
    }
    let mut observation_ids = BTreeSet::new();
    if observations.iter().any(|observation| {
        observation.observation_id.trim().is_empty()
            || observation.candidate_id.trim().is_empty()
            || observation.uncertainty_milli > 1_000
            || !candidate_ids.contains(&observation.candidate_id)
            || !observation_ids.insert(observation.observation_id.clone())
            || observation.artifact.validate().is_err()
    }) {
        return Err(RobustActiveLearningError::InvalidObservation(
            "observation ids, candidate bindings, uncertainty, and local artifacts are required"
                .into(),
        ));
    }
    Ok(())
}

fn weighted_mean(values: &[(i64, u64)]) -> i64 {
    let total = values
        .iter()
        .map(|(_, weight)| *weight as i128)
        .sum::<i128>();
    if total == 0 {
        0
    } else {
        (values
            .iter()
            .map(|(value, weight)| i128::from(*value) * i128::from(*weight))
            .sum::<i128>()
            / total)
            .clamp(i128::from(i64::MIN), i128::from(i64::MAX)) as i64
    }
}

/// Compile a conservative next assay batch across competing local mechanistic surrogates.
pub fn plan_glioma_robust_active_learning(
    request: &RobustActiveLearningRequest,
    candidates: &[RobustActiveLearningCandidate],
    observations: &[RobustActiveLearningObservation],
) -> Result<RobustActiveLearningPlan, RobustActiveLearningError> {
    validate_request(request, candidates, observations)?;
    let mut ordered = candidates.iter().collect::<Vec<_>>();
    ordered.sort_by(|left, right| left.candidate_id.cmp(&right.candidate_id));
    let candidate_map = ordered
        .iter()
        .map(|candidate| (candidate.candidate_id.clone(), *candidate))
        .collect::<BTreeMap<_, _>>();
    let mut observation_order = observations
        .iter()
        .map(|observation| observation.observation_id.clone())
        .collect::<Vec<_>>();
    observation_order.sort();
    let mut scores = Vec::new();
    let mut ranking = Vec::new();
    let mut uncertainty = BTreeSet::new();
    let mut negative = BTreeSet::new();
    let replicate_counts = observations
        .iter()
        .fold(BTreeMap::new(), |mut map, observation| {
            *map.entry(observation.candidate_id.clone())
                .or_insert(0_usize) += 1;
            map
        });
    for candidate in &ordered {
        let direct = observations
            .iter()
            .filter(|observation| observation.candidate_id == candidate.candidate_id)
            .collect::<Vec<_>>();
        let direct_count = direct.len();
        let direct_mean = if direct.is_empty() {
            None
        } else {
            Some(
                direct
                    .iter()
                    .map(|observation| observation.outcome_milli)
                    .sum::<i64>()
                    / direct_count as i64,
            )
        };
        let direct_spread = direct
            .iter()
            .map(|observation| observation.outcome_milli)
            .max()
            .unwrap_or(0)
            .saturating_sub(
                direct
                    .iter()
                    .map(|observation| observation.outcome_milli)
                    .min()
                    .unwrap_or(0),
            )
            .unsigned_abs()
            .min(1_000) as u32;
        let mut predictions = Vec::new();
        let mut model_support = 0_u16;
        for model in &request.models {
            if model.reliability_milli < request.min_model_reliability_milli {
                continue;
            }
            let dot = model
                .feature_weights
                .iter()
                .zip(&candidate.feature_vector)
                .map(|(weight, value)| i128::from(*weight) * i128::from(*value))
                .sum::<i128>();
            let baseline = (i128::from(model.intercept_milli) + dot)
                .clamp(i128::from(i64::MIN), i128::from(i64::MAX))
                as i64;
            let prediction = if let Some(observed) = direct_mean {
                ((i128::from(baseline) * i128::from(model.reliability_milli)
                    + i128::from(observed) * 1_000)
                    / i128::from(model.reliability_milli.saturating_add(1_000)))
                .clamp(i128::from(i64::MIN), i128::from(i64::MAX)) as i64
            } else {
                baseline
            };
            predictions.push((signed(request.direction, prediction), model));
            model_support = model_support.saturating_add(1);
        }
        let weighted = weighted_mean(
            &predictions
                .iter()
                .map(|(prediction, model)| {
                    (
                        *prediction,
                        u64::from(model.prior_weight_milli) * u64::from(model.reliability_milli),
                    )
                })
                .collect::<Vec<_>>(),
        );
        let minimum = predictions
            .iter()
            .map(|(value, _)| *value)
            .min()
            .unwrap_or(0);
        let maximum = predictions
            .iter()
            .map(|(value, _)| *value)
            .max()
            .unwrap_or(0);
        let disagreement = maximum.saturating_sub(minimum).unsigned_abs().min(1_000) as u32;
        let model_residual = predictions
            .iter()
            .map(|(value, model)| {
                value
                    .saturating_sub(weighted)
                    .unsigned_abs()
                    .max(u64::from(model.residual_milli))
            })
            .max()
            .unwrap_or(1_000)
            .min(1_000) as u16;
        let observed_uncertainty = direct
            .iter()
            .map(|observation| observation.uncertainty_milli)
            .max()
            .unwrap_or(0);
        let posterior_uncertainty = model_residual
            .max(observed_uncertainty)
            .max(direct_spread.min(1_000) as u16)
            .max(disagreement.min(1_000) as u16);
        let lower_tail = weighted.saturating_sub(i64::from(posterior_uncertainty));
        let expected_information = posterior_uncertainty.max(disagreement.min(1_000) as u16);
        let acquisition = (i128::from(lower_tail) * i128::from(request.lower_tail_weight_milli)
            + i128::from(disagreement) * i128::from(request.disagreement_weight_milli)
            + i128::from(expected_information) * i128::from(request.information_weight_milli))
            / SCORE_SCALE
            - i128::from(candidate.cost_units) * i128::from(request.cost_penalty_milli)
            - i128::from(candidate.risk_milli) * i128::from(request.risk_penalty_milli)
                / SCORE_SCALE;
        let rationale = if model_support == 0 {
            "no surrogate meets the reliability floor; candidate is blocked rather than extrapolated".into()
        } else if direct_count >= 2
            && direct_spread >= u32::from(request.min_model_reliability_milli)
        {
            "direct observations contradict; lower-tail utility is withheld pending resolution"
                .into()
        } else if direct_count == 0 {
            "prior-weighted model ensemble exposes exploration value and conservative tail risk"
                .into()
        } else {
            "local observations are shrunk toward a reliability-weighted ensemble with disagreement penalty".into()
        };
        if direct_count >= 2 && direct_spread >= u32::from(request.min_model_reliability_milli) {
            uncertainty.insert(format!(
                "{}:contradictory-spread-{}",
                candidate.candidate_id, direct_spread
            ));
        }
        scores.push(RobustActiveLearningScore {
            candidate_id: candidate.candidate_id.clone(),
            weighted_mean_milli: weighted,
            lower_tail_milli: lower_tail,
            model_disagreement_milli: disagreement,
            posterior_uncertainty_milli: posterior_uncertainty,
            expected_information_milli: expected_information,
            model_support_count: model_support,
            direct_observation_count: direct_count.min(u16::MAX as usize) as u16,
            acquisition_milli: acquisition.clamp(i128::from(i64::MIN), i128::from(i64::MAX)) as i64,
            disposition: RobustActiveLearningCandidateDisposition::Unresolved,
            rationale,
        });
        ranking.push(scores.len() - 1);
    }
    ranking.sort_by(|left, right| {
        scores[*right]
            .acquisition_milli
            .cmp(&scores[*left].acquisition_milli)
            .then_with(|| scores[*left].candidate_id.cmp(&scores[*right].candidate_id))
    });
    let mut selected = Vec::new();
    let mut deferred = Vec::new();
    let mut blocked = Vec::new();
    let mut unresolved = Vec::new();
    let mut groups = BTreeSet::new();
    let mut budget = request.budget_units;
    for index in ranking {
        let score = &mut scores[index];
        let candidate = candidate_map[&score.candidate_id];
        let count = replicate_counts
            .get(&candidate.candidate_id)
            .copied()
            .unwrap_or(0);
        let reason = if score.model_support_count == 0 {
            blocked.push(candidate.candidate_id.clone());
            score.disposition = RobustActiveLearningCandidateDisposition::Blocked;
            Some("reliability-floor-blocked")
        } else if candidate.risk_milli > request.max_risk_milli {
            blocked.push(candidate.candidate_id.clone());
            score.disposition = RobustActiveLearningCandidateDisposition::Blocked;
            Some("risk-ceiling-blocked")
        } else if count >= usize::from(candidate.max_replicates) {
            blocked.push(candidate.candidate_id.clone());
            score.disposition = RobustActiveLearningCandidateDisposition::Blocked;
            Some("replicate-ceiling-reached")
        } else if candidate.cost_units > budget {
            blocked.push(candidate.candidate_id.clone());
            score.disposition = RobustActiveLearningCandidateDisposition::Blocked;
            Some("budget-blocked")
        } else if count > 0
            && score.direct_observation_count >= 2
            && score.posterior_uncertainty_milli >= request.min_model_reliability_milli
        {
            unresolved.push(candidate.candidate_id.clone());
            score.disposition = RobustActiveLearningCandidateDisposition::Unresolved;
            Some("contradiction-hold")
        } else if selected.len() >= request.max_selections {
            deferred.push(candidate.candidate_id.clone());
            score.disposition = RobustActiveLearningCandidateDisposition::Deferred;
            Some("selection-cap-deferred")
        } else if groups.contains(&candidate.redundancy_group) {
            deferred.push(candidate.candidate_id.clone());
            score.disposition = RobustActiveLearningCandidateDisposition::Deferred;
            Some("redundancy-group-deferred")
        } else {
            selected.push(candidate.candidate_id.clone());
            groups.insert(candidate.redundancy_group.clone());
            budget = budget.saturating_sub(candidate.cost_units);
            score.disposition = RobustActiveLearningCandidateDisposition::Selected;
            None
        };
        if reason == Some("budget-blocked") {
            negative.insert(format!("{}:budget-blocked", candidate.candidate_id));
        }
    }
    let disposition = if selected.is_empty() && blocked.len() == scores.len() {
        RobustActiveLearningDisposition::NoCandidates
    } else if !unresolved.is_empty() || !blocked.is_empty() || !deferred.is_empty() {
        RobustActiveLearningDisposition::Partial
    } else if selected.is_empty() {
        RobustActiveLearningDisposition::Unresolved
    } else {
        RobustActiveLearningDisposition::Qualified
    };
    let mut plan = RobustActiveLearningPlan {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        objective: request.objective.clone(),
        model_system: request.model_system,
        candidate_order: ordered
            .iter()
            .map(|candidate| candidate.candidate_id.clone())
            .collect(),
        observation_order,
        scores,
        selected_order: selected,
        deferred_order: deferred,
        blocked_order: blocked,
        unresolved_order: unresolved,
        remaining_budget_units: budget,
        negative_evidence: negative.into_iter().collect(),
        uncertainty: uncertainty.into_iter().collect(),
        disposition,
        digest: ContentHash::of_bytes(b"unsealed-glioma-robust-active-learning"),
    };
    plan.digest = ContentHash::of_value(&digest_input(&plan))
        .map_err(|error| RobustActiveLearningError::Digest(error.to_string()))?;
    plan.validate()?;
    Ok(plan)
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

    fn request() -> RobustActiveLearningRequest {
        RobustActiveLearningRequest {
            objective: "choose a robust invasion assay".into(),
            model_system: GliomaModelSystem::Organoid,
            direction: ActiveLearningDirection::Maximize,
            budget_units: 4,
            max_selections: 2,
            min_observations_per_candidate: 1,
            lower_tail_weight_milli: 600,
            disagreement_weight_milli: 300,
            information_weight_milli: 100,
            cost_penalty_milli: 1,
            risk_penalty_milli: 1,
            max_risk_milli: 800,
            min_model_reliability_milli: 500,
            models: vec![
                RobustActiveLearningModel {
                    model_id: "mechanistic".into(),
                    prior_weight_milli: 600,
                    intercept_milli: 100,
                    feature_weights: vec![4, 1],
                    residual_milli: 50,
                    reliability_milli: 900,
                },
                RobustActiveLearningModel {
                    model_id: "spatial".into(),
                    prior_weight_milli: 400,
                    intercept_milli: 50,
                    feature_weights: vec![1, 4],
                    residual_milli: 80,
                    reliability_milli: 800,
                },
            ],
        }
    }

    fn candidates() -> Vec<RobustActiveLearningCandidate> {
        vec![
            RobustActiveLearningCandidate {
                candidate_id: "egfr".into(),
                mechanism_id: "egfr".into(),
                feature_vector: vec![100, 0],
                cost_units: 2,
                risk_milli: 100,
                max_replicates: 2,
                redundancy_group: "receptor".into(),
                output_schema: "Assay1@1".into(),
            },
            RobustActiveLearningCandidate {
                candidate_id: "matrix".into(),
                mechanism_id: "matrix".into(),
                feature_vector: vec![0, 100],
                cost_units: 2,
                risk_milli: 100,
                max_replicates: 2,
                redundancy_group: "matrix".into(),
                output_schema: "Assay1@1".into(),
            },
        ]
    }

    #[test]
    fn ensemble_selects_diverse_candidates_and_replays() {
        let first = plan_glioma_robust_active_learning(&request(), &candidates(), &[]).unwrap();
        let mut reversed = candidates();
        reversed.reverse();
        let second = plan_glioma_robust_active_learning(&request(), &reversed, &[]).unwrap();
        assert_eq!(first, second);
        assert_eq!(first.selected_order.len(), 2);
        first.validate().unwrap();
    }

    #[test]
    fn contradictory_direct_observations_are_held_explicitly() {
        let mut candidates = candidates();
        candidates[0].max_replicates = 3;
        let observations = vec![
            RobustActiveLearningObservation {
                observation_id: "a".into(),
                candidate_id: "egfr".into(),
                outcome_milli: 900,
                uncertainty_milli: 20,
                artifact: artifact("a"),
            },
            RobustActiveLearningObservation {
                observation_id: "b".into(),
                candidate_id: "egfr".into(),
                outcome_milli: -900,
                uncertainty_milli: 20,
                artifact: artifact("b"),
            },
        ];
        let plan =
            plan_glioma_robust_active_learning(&request(), &candidates, &observations).unwrap();
        assert!(plan
            .uncertainty
            .iter()
            .any(|item| item.starts_with("egfr:contradictory")));
        assert!(plan.unresolved_order.contains(&"egfr".into()));
    }

    #[test]
    fn reliability_floor_blocks_unsupported_models() {
        let mut request = request();
        request.min_model_reliability_milli = 950;
        let plan = plan_glioma_robust_active_learning(&request, &candidates(), &[]).unwrap();
        assert_eq!(
            plan.disposition,
            RobustActiveLearningDisposition::NoCandidates
        );
        assert_eq!(plan.blocked_order.len(), 2);
        assert!(plan
            .scores
            .iter()
            .all(|score| score.model_support_count == 0));
    }
}
