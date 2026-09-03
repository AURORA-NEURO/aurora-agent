//! Mechanism-aware active learning for bounded preclinical glioma assays.
//!
//! This module is a deterministic, local-first surrogate optimizer. It combines observations
//! from the same candidate and nearby candidates in a declared feature space using an integer
//! inverse-distance kernel, estimates a robust residual uncertainty, and ranks the next assay by
//! exploitation, exploration, cost, risk, and redundancy constraints. It is deliberately not a
//! black-box efficacy claim: sparse support, disagreement, null outcomes, and missing observations
//! remain visible in the plan and never become confidence by imputation.

use crate::glioma_engine::{GliomaModelSystem, LocalArtifactRef};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P06-F12";
pub const OUTPUT_SCHEMA: &str = "GliomaActiveLearning1@1";
pub const MAX_CANDIDATES: usize = 4_096;
pub const MAX_OBSERVATIONS: usize = 65_536;
pub const MAX_DIMENSIONS: usize = 64;
const SCORE_SCALE: i128 = 1_000;
const VALUE_LIMIT: i64 = 1_000_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActiveLearningDirection {
    Maximize,
    Minimize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ActiveLearningRequest {
    pub objective: String,
    pub model_system: GliomaModelSystem,
    pub direction: ActiveLearningDirection,
    pub budget_units: u32,
    pub max_selections: usize,
    pub min_observations_per_candidate: usize,
    pub exploration_weight_milli: u16,
    pub exploitation_weight_milli: u16,
    pub cost_penalty_milli: u16,
    pub risk_penalty_milli: u16,
    pub max_risk_milli: u16,
    pub min_uncertainty_milli: u16,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ActiveLearningCandidate {
    pub candidate_id: String,
    pub mechanism_id: String,
    pub feature_vector: Vec<i64>,
    pub cost_units: u32,
    pub risk_milli: u16,
    pub max_replicates: usize,
    pub redundancy_group: String,
    pub output_schema: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ActiveLearningObservation {
    pub observation_id: String,
    pub candidate_id: String,
    pub outcome_milli: i64,
    pub uncertainty_milli: u16,
    pub artifact: LocalArtifactRef,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActiveLearningCandidateDisposition {
    Selected,
    Deferred,
    Blocked,
    Unresolved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ActiveLearningScore {
    pub candidate_id: String,
    pub posterior_mean_milli: i64,
    pub posterior_uncertainty_milli: u16,
    pub nearest_observation_count: usize,
    pub kernel_weight_milli: u32,
    pub acquisition_milli: i64,
    pub disposition: ActiveLearningCandidateDisposition,
    pub rationale: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActiveLearningDisposition {
    Qualified,
    Partial,
    NoCandidates,
    Unresolved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ActiveLearningPlan {
    pub feature_id: String,
    pub output_schema: String,
    pub objective: String,
    pub model_system: GliomaModelSystem,
    pub candidate_order: Vec<String>,
    pub observation_order: Vec<String>,
    pub scores: Vec<ActiveLearningScore>,
    pub selected_order: Vec<String>,
    pub deferred_order: Vec<String>,
    pub blocked_order: Vec<String>,
    pub unresolved_order: Vec<String>,
    pub remaining_budget_units: u32,
    pub negative_evidence: Vec<String>,
    pub uncertainty: Vec<String>,
    pub disposition: ActiveLearningDisposition,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ActiveLearningError {
    #[error("active-learning request is invalid: {0}")]
    InvalidRequest(String),
    #[error("active-learning candidate is invalid: {0}")]
    InvalidCandidate(String),
    #[error("active-learning observation is invalid: {0}")]
    InvalidObservation(String),
    #[error("active-learning output is invalid: {0}")]
    InvalidOutput(String),
    #[error("active-learning digest failed: {0}")]
    Digest(String),
}

fn digest_input(plan: &ActiveLearningPlan) -> serde_json::Value {
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

fn canonical(values: &[String]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

impl ActiveLearningPlan {
    pub fn validate(&self) -> Result<(), ActiveLearningError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || self.objective.trim().is_empty()
            || self
                .candidate_order
                .windows(2)
                .any(|pair| pair[0] >= pair[1])
            || self
                .observation_order
                .windows(2)
                .any(|pair| pair[0] >= pair[1])
            || !canonical(&self.blocked_order)
            || !canonical(&self.unresolved_order)
            || !canonical(&self.negative_evidence)
            || !canonical(&self.uncertainty)
            || self.scores.len() != self.candidate_order.len()
            || self.scores.iter().any(|score| {
                score.candidate_id.trim().is_empty()
                    || score.rationale.trim().is_empty()
                    || score.posterior_uncertainty_milli > 1_000
                    || score.kernel_weight_milli > 1_000_000
            })
        {
            return Err(ActiveLearningError::InvalidOutput(
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
            return Err(ActiveLearningError::InvalidOutput(
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
            return Err(ActiveLearningError::InvalidOutput(
                "selected, deferred, blocked, and unresolved partitions do not reconcile".into(),
            ));
        }
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| ActiveLearningError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(ActiveLearningError::InvalidOutput(
                "active-learning digest is not content-addressed".into(),
            ));
        }
        Ok(())
    }
}

fn validate_request(
    request: &ActiveLearningRequest,
    candidates: &[ActiveLearningCandidate],
    observations: &[ActiveLearningObservation],
) -> Result<usize, ActiveLearningError> {
    let weights =
        u32::from(request.exploration_weight_milli) + u32::from(request.exploitation_weight_milli);
    if request.objective.trim().is_empty()
        || request.budget_units == 0
        || request.max_selections == 0
        || candidates.is_empty()
        || candidates.len() > MAX_CANDIDATES
        || observations.len() > MAX_OBSERVATIONS
        || request.min_observations_per_candidate == 0
        || weights != 1_000
        || request.max_risk_milli > 1_000
        || request.min_uncertainty_milli > 1_000
    {
        return Err(ActiveLearningError::InvalidRequest(
            "objective, bounded candidates, positive budget/selection/replicate floors, and weights summing to 1,000 are required".into(),
        ));
    }
    let dimensions = candidates[0].feature_vector.len();
    if dimensions == 0 || dimensions > MAX_DIMENSIONS {
        return Err(ActiveLearningError::InvalidCandidate(
            "feature vectors must have a shared non-zero bounded dimension".into(),
        ));
    }
    let mut candidate_ids = BTreeSet::new();
    for candidate in candidates {
        if candidate.candidate_id.trim().is_empty()
            || candidate.mechanism_id.trim().is_empty()
            || candidate.redundancy_group.trim().is_empty()
            || candidate.output_schema.trim().is_empty()
            || candidate.feature_vector.len() != dimensions
            || candidate
                .feature_vector
                .iter()
                .any(|value| value.unsigned_abs() > VALUE_LIMIT as u64)
            || candidate.cost_units == 0
            || candidate.risk_milli > 1_000
            || candidate.max_replicates == 0
            || !candidate_ids.insert(candidate.candidate_id.clone())
        {
            return Err(ActiveLearningError::InvalidCandidate(
                "candidate identity, shared feature dimensions, bounded values, cost/risk, replicate ceiling, and redundancy group are required".into(),
            ));
        }
    }
    let mut observation_ids = BTreeSet::new();
    for observation in observations {
        if observation.observation_id.trim().is_empty()
            || !candidate_ids.contains(&observation.candidate_id)
            || observation.outcome_milli.unsigned_abs() > VALUE_LIMIT as u64
            || observation.uncertainty_milli > 1_000
            || !observation_ids.insert(observation.observation_id.clone())
        {
            return Err(ActiveLearningError::InvalidObservation(
                "observation identity, candidate binding, bounded outcome/uncertainty, and uniqueness are required".into(),
            ));
        }
        observation
            .artifact
            .validate()
            .map_err(|error| ActiveLearningError::InvalidObservation(error.to_string()))?;
    }
    Ok(dimensions)
}

fn distance(left: &[i64], right: &[i64]) -> u64 {
    left.iter()
        .zip(right)
        .map(|(a, b)| a.saturating_sub(*b).unsigned_abs())
        .fold(0_u64, |sum, value| sum.saturating_add(value))
}

fn signed_outcome(direction: ActiveLearningDirection, outcome: i64) -> i64 {
    match direction {
        ActiveLearningDirection::Maximize => outcome,
        ActiveLearningDirection::Minimize => outcome.saturating_neg(),
    }
}

fn weighted_mean(values: &[(i64, u64)]) -> i64 {
    let weight = values
        .iter()
        .map(|(_, weight)| *weight as i128)
        .sum::<i128>();
    if weight == 0 {
        0
    } else {
        (values
            .iter()
            .map(|(value, weight)| i128::from(*value) * i128::from(*weight))
            .sum::<i128>()
            / weight)
            .clamp(i128::from(i64::MIN), i128::from(i64::MAX)) as i64
    }
}

fn score_candidate(
    request: &ActiveLearningRequest,
    candidate: &ActiveLearningCandidate,
    candidates: &BTreeMap<String, &ActiveLearningCandidate>,
    observations: &[ActiveLearningObservation],
) -> ActiveLearningScore {
    let mut weighted = Vec::new();
    let mut nearest_count = 0_usize;
    let mut kernel_sum = 0_u64;
    for observation in observations {
        let Some(observed_candidate) = candidates.get(&observation.candidate_id) else {
            continue;
        };
        let distance = distance(
            &candidate.feature_vector,
            &observed_candidate.feature_vector,
        );
        let weight = 1_000_000_u64 / distance.saturating_add(1);
        if weight == 0 {
            continue;
        }
        nearest_count += 1;
        kernel_sum = kernel_sum.saturating_add(weight);
        weighted.push((
            signed_outcome(request.direction, observation.outcome_milli),
            weight,
        ));
    }
    let posterior_mean = weighted_mean(&weighted);
    let residuals = weighted
        .iter()
        .map(|(value, weight)| (value.saturating_sub(posterior_mean).unsigned_abs(), *weight))
        .collect::<Vec<_>>();
    // The maximum residual is intentionally conservative: a nearby contradictory observation
    // must widen uncertainty even when its kernel weight is small, rather than being averaged
    // away into a confident-looking assay recommendation.
    let residual = residuals
        .iter()
        .map(|(value, _)| *value)
        .max()
        .unwrap_or(0)
        .min(1_000_000);
    let measurement = observations
        .iter()
        .filter(|observation| observation.candidate_id == candidate.candidate_id)
        .map(|observation| u64::from(observation.uncertainty_milli))
        .max()
        .unwrap_or(0);
    let distance_uncertainty = if nearest_count == 0 {
        1_000_000
    } else {
        (1_000_000_u64 / kernel_sum.max(1)).min(1_000_000)
    };
    let uncertainty = residual
        .max(measurement)
        .max(distance_uncertainty)
        .min(1_000) as u16;
    let exploit = i128::from(posterior_mean)
        .saturating_mul(i128::from(request.exploitation_weight_milli))
        / SCORE_SCALE;
    let explore = i128::from(uncertainty)
        .saturating_mul(i128::from(request.exploration_weight_milli))
        / SCORE_SCALE;
    let cost_penalty =
        i128::from(candidate.cost_units).saturating_mul(i128::from(request.cost_penalty_milli));
    let risk_penalty = i128::from(candidate.risk_milli)
        .saturating_mul(i128::from(request.risk_penalty_milli))
        / SCORE_SCALE;
    let acquisition = exploit
        .saturating_add(explore)
        .saturating_sub(cost_penalty)
        .saturating_sub(risk_penalty)
        .clamp(i128::from(i64::MIN), i128::from(i64::MAX)) as i64;
    let rationale = if nearest_count == 0 {
        "no local observations; exploration uncertainty is explicit and requires a first assay"
            .into()
    } else if residual > u64::from(request.min_uncertainty_milli) {
        "nearby observations disagree; uncertainty-weighted exploration remains active".into()
    } else {
        "kernel-weighted local posterior balances expected direction against bounded uncertainty"
            .into()
    };
    ActiveLearningScore {
        candidate_id: candidate.candidate_id.clone(),
        posterior_mean_milli: posterior_mean,
        posterior_uncertainty_milli: uncertainty,
        nearest_observation_count: nearest_count,
        kernel_weight_milli: kernel_sum.min(1_000_000) as u32,
        acquisition_milli: acquisition,
        disposition: ActiveLearningCandidateDisposition::Unresolved,
        rationale,
    }
}

/// Compile a deterministic next-assay plan from local observations and candidate interventions.
pub fn plan_glioma_active_learning(
    request: &ActiveLearningRequest,
    candidates: &[ActiveLearningCandidate],
    observations: &[ActiveLearningObservation],
) -> Result<ActiveLearningPlan, ActiveLearningError> {
    validate_request(request, candidates, observations)?;
    let mut ordered_candidates = candidates.iter().collect::<Vec<_>>();
    ordered_candidates.sort_by(|left, right| left.candidate_id.cmp(&right.candidate_id));
    let candidate_map = ordered_candidates
        .iter()
        .map(|candidate| (candidate.candidate_id.clone(), *candidate))
        .collect::<BTreeMap<_, _>>();
    let mut observation_order = observations
        .iter()
        .map(|observation| observation.observation_id.clone())
        .collect::<Vec<_>>();
    observation_order.sort();
    let mut scores = ordered_candidates
        .iter()
        .map(|candidate| score_candidate(request, candidate, &candidate_map, observations))
        .collect::<Vec<_>>();
    let mut ranking = scores.iter().enumerate().collect::<Vec<_>>();
    ranking.sort_by(|(_, left), (_, right)| {
        right
            .acquisition_milli
            .cmp(&left.acquisition_milli)
            .then_with(|| left.candidate_id.cmp(&right.candidate_id))
    });
    let replicate_counts = observations
        .iter()
        .fold(BTreeMap::new(), |mut counts, observation| {
            *counts
                .entry(observation.candidate_id.clone())
                .or_insert(0_usize) += 1;
            counts
        });
    let mut selected = Vec::new();
    let mut deferred = Vec::new();
    let mut blocked = Vec::new();
    let mut unresolved = Vec::new();
    let mut negative = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    let mut remaining_budget = request.budget_units;
    let mut groups = BTreeSet::new();
    for (index, score) in &ranking {
        let candidate = ordered_candidates[*index];
        let count = replicate_counts
            .get(&candidate.candidate_id)
            .copied()
            .unwrap_or(0);
        let mut reason = None;
        if candidate.risk_milli > request.max_risk_milli {
            reason = Some("risk-ceiling-blocked");
            blocked.push(candidate.candidate_id.clone());
        } else if count >= candidate.max_replicates {
            reason = Some("replicate-ceiling-reached");
            blocked.push(candidate.candidate_id.clone());
        } else if candidate.cost_units > remaining_budget {
            reason = Some("budget-blocked");
            blocked.push(candidate.candidate_id.clone());
        } else if selected.len() >= request.max_selections {
            reason = Some("selection-cap-deferred");
            deferred.push(candidate.candidate_id.clone());
        } else if groups.contains(&candidate.redundancy_group) {
            reason = Some("redundancy-group-deferred");
            deferred.push(candidate.candidate_id.clone());
        } else if score.posterior_uncertainty_milli > request.min_uncertainty_milli
            && score.nearest_observation_count > 0
        {
            unresolved.push(candidate.candidate_id.clone());
            uncertainty.insert(format!(
                "{}:uncertainty-{}",
                candidate.candidate_id, score.posterior_uncertainty_milli
            ));
            reason = Some("uncertainty-hold");
        } else {
            selected.push(candidate.candidate_id.clone());
            remaining_budget = remaining_budget.saturating_sub(candidate.cost_units);
            groups.insert(candidate.redundancy_group.clone());
        }
        if let Some(reason) = reason {
            if reason.contains("budget") {
                negative.insert(format!("{}:{reason}", candidate.candidate_id));
            }
        }
    }
    for score in &mut scores {
        score.disposition = if selected.contains(&score.candidate_id) {
            ActiveLearningCandidateDisposition::Selected
        } else if deferred.contains(&score.candidate_id) {
            ActiveLearningCandidateDisposition::Deferred
        } else if blocked.contains(&score.candidate_id) {
            ActiveLearningCandidateDisposition::Blocked
        } else {
            ActiveLearningCandidateDisposition::Unresolved
        };
    }
    let disposition = if selected.is_empty() && blocked.len() == scores.len() {
        ActiveLearningDisposition::NoCandidates
    } else if !unresolved.is_empty() || !blocked.is_empty() {
        ActiveLearningDisposition::Partial
    } else if selected.is_empty() {
        ActiveLearningDisposition::Unresolved
    } else {
        ActiveLearningDisposition::Qualified
    };
    let mut plan = ActiveLearningPlan {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        objective: request.objective.clone(),
        model_system: request.model_system,
        candidate_order: ordered_candidates
            .iter()
            .map(|candidate| candidate.candidate_id.clone())
            .collect(),
        observation_order,
        scores,
        selected_order: selected,
        deferred_order: deferred,
        blocked_order: blocked,
        unresolved_order: unresolved,
        remaining_budget_units: remaining_budget,
        negative_evidence: negative.into_iter().collect(),
        uncertainty: uncertainty.into_iter().collect(),
        disposition,
        digest: ContentHash::of_bytes(b"unsealed-glioma-active-learning"),
    };
    plan.digest = ContentHash::of_value(&digest_input(&plan))
        .map_err(|error| ActiveLearningError::Digest(error.to_string()))?;
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
            content_type: "application/vnd.aurora.glioma-active-learning+json".into(),
            local_only: true,
            contains_human_data: false,
            contains_direct_identifiers: false,
        }
    }

    fn request() -> ActiveLearningRequest {
        ActiveLearningRequest {
            objective: "select the next invasion mechanism assay".into(),
            model_system: GliomaModelSystem::Organoid,
            direction: ActiveLearningDirection::Maximize,
            budget_units: 4,
            max_selections: 2,
            min_observations_per_candidate: 1,
            exploration_weight_milli: 400,
            exploitation_weight_milli: 600,
            cost_penalty_milli: 1,
            risk_penalty_milli: 1,
            max_risk_milli: 800,
            min_uncertainty_milli: 900,
        }
    }

    fn candidates() -> Vec<ActiveLearningCandidate> {
        vec![
            ActiveLearningCandidate {
                candidate_id: "egfr".into(),
                mechanism_id: "egfr-signaling".into(),
                feature_vector: vec![100, 0],
                cost_units: 2,
                risk_milli: 100,
                max_replicates: 3,
                redundancy_group: "receptor".into(),
                output_schema: "Assay1@1".into(),
            },
            ActiveLearningCandidate {
                candidate_id: "matrix".into(),
                mechanism_id: "matrix-remodeling".into(),
                feature_vector: vec![0, 100],
                cost_units: 2,
                risk_milli: 100,
                max_replicates: 3,
                redundancy_group: "matrix".into(),
                output_schema: "Assay1@1".into(),
            },
            ActiveLearningCandidate {
                candidate_id: "unsafe".into(),
                mechanism_id: "unsafe".into(),
                feature_vector: vec![50, 50],
                cost_units: 1,
                risk_milli: 900,
                max_replicates: 3,
                redundancy_group: "unsafe".into(),
                output_schema: "Assay1@1".into(),
            },
        ]
    }

    fn observations() -> Vec<ActiveLearningObservation> {
        vec![ActiveLearningObservation {
            observation_id: "obs-egfr".into(),
            candidate_id: "egfr".into(),
            outcome_milli: 700,
            uncertainty_milli: 20,
            artifact: artifact("obs-egfr"),
        }]
    }

    #[test]
    fn selects_safe_candidates_and_is_replay_stable() {
        let first =
            plan_glioma_active_learning(&request(), &candidates(), &observations()).unwrap();
        let mut reversed = candidates();
        reversed.reverse();
        let second = plan_glioma_active_learning(&request(), &reversed, &observations()).unwrap();
        assert_eq!(first, second);
        assert_eq!(first.selected_order, vec!["matrix", "egfr"]);
        assert_eq!(first.blocked_order, vec!["unsafe"]);
        assert_eq!(first.disposition, ActiveLearningDisposition::Partial);
        first.validate().unwrap();
    }

    #[test]
    fn contradictory_nearby_support_remains_unresolved() {
        let mut observations = observations();
        observations.push(ActiveLearningObservation {
            observation_id: "obs-matrix".into(),
            candidate_id: "matrix".into(),
            outcome_milli: -600,
            uncertainty_milli: 100,
            artifact: artifact("obs-matrix"),
        });
        let mut request = request();
        request.min_uncertainty_milli = 100;
        let plan = plan_glioma_active_learning(&request, &candidates(), &observations).unwrap();
        assert!(plan
            .uncertainty
            .iter()
            .any(|item| item.starts_with("matrix:")));
        assert!(plan.unresolved_order.contains(&"matrix".into()));
        assert_eq!(plan.disposition, ActiveLearningDisposition::Partial);
    }

    #[test]
    fn missing_observations_are_explicit_exploration_not_confidence() {
        let plan = plan_glioma_active_learning(&request(), &candidates(), &[]).unwrap();
        assert!(plan
            .scores
            .iter()
            .all(|score| score.nearest_observation_count == 0));
        assert!(plan
            .scores
            .iter()
            .all(|score| score.rationale.contains("no local observations")));
    }
}
