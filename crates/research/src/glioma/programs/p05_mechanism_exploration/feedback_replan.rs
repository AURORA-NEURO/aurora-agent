//! Observation-driven mechanism frontier replanning for preclinical glioma research.
//!
//! A closed-loop frontier is useful only if returned local outcomes change what the engine does
//! next. This feature consumes a previously validated mechanism frontier, typed local outcome
//! observations, and refreshed action economics, then deterministically reprioritizes the next
//! bounded batch. Contradictions increase discrimination pressure, supported outcomes move toward
//! validation, unresolved outcomes retain exploration pressure, and unobserved actions remain
//! explicit omissions. The module never fabricates outcomes or dispatches an assay.

use super::closed_loop::MechanismClosedLoopPlan;
use super::evidence_assimilation::AssimilatedMechanismStatus;
use crate::glioma_engine::LocalArtifactRef;
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P05-F12";
pub const OUTPUT_SCHEMA: &str = "GliomaMechanismFeedbackReplan1@1";
pub const MAX_ACTIONS: usize = 1_024;
pub const MAX_OBSERVATIONS: usize = 16_384;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MechanismFeedbackOutcome {
    Supported,
    Contradicted,
    Unresolved,
    NotObserved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MechanismFeedbackCandidate {
    pub action_id: String,
    pub mechanism_id: String,
    pub cost_units: u32,
    pub risk_milli: u16,
    pub requires_approval: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MechanismFeedbackObservation {
    pub observation_id: String,
    pub action_id: String,
    pub mechanism_id: String,
    pub outcome: MechanismFeedbackOutcome,
    pub observed_information_gain_milli: u16,
    pub uncertainty_milli: u16,
    pub artifact: LocalArtifactRef,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MechanismFeedbackReplanRequest {
    pub objective: String,
    pub prior_plan: MechanismClosedLoopPlan,
    pub candidates: Vec<MechanismFeedbackCandidate>,
    pub observations: Vec<MechanismFeedbackObservation>,
    pub budget_units: u64,
    pub max_actions: usize,
    pub minimum_priority_milli: u16,
    pub maximum_risk_milli: u16,
    pub allow_approval_required: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MechanismFeedbackActionScore {
    pub action_id: String,
    pub mechanism_id: String,
    pub prior_priority_milli: u16,
    pub feedback_adjustment_milli: i16,
    pub observed_information_gain_milli: u16,
    pub posterior_priority_milli: u16,
    pub eligible: bool,
    pub exclusion_reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MechanismFeedbackDecision {
    pub mechanism_id: String,
    pub prior_status: AssimilatedMechanismStatus,
    pub outcome_order: Vec<MechanismFeedbackOutcome>,
    pub next_action_id: Option<String>,
    pub rationale: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MechanismFeedbackReplanDisposition {
    Ready,
    Partial,
    Blocked,
    NoEligibleActions,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MechanismFeedbackReplan {
    pub feature_id: String,
    pub output_schema: String,
    pub objective: String,
    pub prior_plan_digest: ContentHash,
    pub mechanism_order: Vec<String>,
    pub observation_order: Vec<String>,
    pub decisions: Vec<MechanismFeedbackDecision>,
    pub candidate_order: Vec<String>,
    pub ranked_action_order: Vec<String>,
    pub selected_action_order: Vec<String>,
    pub deferred_action_order: Vec<String>,
    pub blocked_action_order: Vec<String>,
    pub scores: Vec<MechanismFeedbackActionScore>,
    pub budget_units: u64,
    pub total_cost_units: u64,
    pub budget_remaining_units: u64,
    pub negative_evidence: Vec<String>,
    pub uncertainty: Vec<String>,
    pub disposition: MechanismFeedbackReplanDisposition,
    pub next_route: String,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum MechanismFeedbackReplanError {
    #[error("mechanism feedback replan request is invalid: {0}")]
    InvalidRequest(String),
    #[error("mechanism feedback replan output is invalid: {0}")]
    InvalidOutput(String),
    #[error("mechanism feedback replan digest failed: {0}")]
    Digest(String),
}

fn canonical<T: Ord>(values: &[T]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn unique_nonempty(values: &[String]) -> bool {
    values.iter().all(|value| !value.trim().is_empty())
        && values.iter().collect::<BTreeSet<_>>().len() == values.len()
}

pub(crate) fn digest_input(output: &MechanismFeedbackReplan) -> serde_json::Value {
    serde_json::json!({
        "feature_id": output.feature_id,
        "output_schema": output.output_schema,
        "objective": output.objective,
        "prior_plan_digest": output.prior_plan_digest,
        "mechanism_order": output.mechanism_order,
        "observation_order": output.observation_order,
        "decisions": output.decisions,
        "candidate_order": output.candidate_order,
        "ranked_action_order": output.ranked_action_order,
        "selected_action_order": output.selected_action_order,
        "deferred_action_order": output.deferred_action_order,
        "blocked_action_order": output.blocked_action_order,
        "scores": output.scores,
        "budget_units": output.budget_units,
        "total_cost_units": output.total_cost_units,
        "budget_remaining_units": output.budget_remaining_units,
        "negative_evidence": output.negative_evidence,
        "uncertainty": output.uncertainty,
        "disposition": output.disposition,
        "next_route": output.next_route,
    })
}

impl MechanismFeedbackReplan {
    pub fn validate(&self) -> Result<(), MechanismFeedbackReplanError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || self.objective.trim().is_empty()
            || self.prior_plan_digest.as_str().len() != 64
            || !canonical(&self.mechanism_order)
            || !unique_nonempty(&self.mechanism_order)
            || !canonical(&self.observation_order)
            || !unique_nonempty(&self.observation_order)
            || !canonical(&self.candidate_order)
            || !canonical(&self.deferred_action_order)
            || !canonical(&self.blocked_action_order)
            || !canonical(&self.negative_evidence)
            || !canonical(&self.uncertainty)
            || !unique_nonempty(&self.candidate_order)
            || !unique_nonempty(&self.ranked_action_order)
            || !unique_nonempty(&self.selected_action_order)
            || !unique_nonempty(&self.deferred_action_order)
            || !unique_nonempty(&self.blocked_action_order)
            || self.next_route.trim().is_empty()
            || self.scores.iter().any(|score| {
                score.action_id.trim().is_empty()
                    || score.mechanism_id.trim().is_empty()
                    || score.prior_priority_milli > 1_000
                    || score.observed_information_gain_milli > 1_000
                    || score.posterior_priority_milli > 1_000
                    || !(-1_000..=1_000).contains(&score.feedback_adjustment_milli)
            })
        {
            return Err(MechanismFeedbackReplanError::InvalidOutput(
                "identity, ordering, route, score bounds, or digest shape is invalid".into(),
            ));
        }
        let score_ids = self
            .scores
            .iter()
            .map(|score| score.action_id.clone())
            .collect::<BTreeSet<_>>();
        let ranked_ids = self
            .ranked_action_order
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>();
        let selected_ids = self
            .selected_action_order
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>();
        let deferred_ids = self
            .deferred_action_order
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>();
        let blocked_ids = self
            .blocked_action_order
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>();
        let partition_union = selected_ids
            .union(&deferred_ids)
            .cloned()
            .collect::<BTreeSet<_>>()
            .union(&blocked_ids)
            .cloned()
            .collect::<BTreeSet<_>>();
        let decision_order = self
            .decisions
            .iter()
            .map(|decision| decision.mechanism_id.clone())
            .collect::<Vec<_>>();
        if self.scores.len() != score_ids.len()
            || self.candidate_order.len() != score_ids.len()
            || score_ids
                != self
                    .candidate_order
                    .iter()
                    .cloned()
                    .collect::<BTreeSet<_>>()
            || ranked_ids != score_ids
            || self.ranked_action_order
                != self
                    .scores
                    .iter()
                    .map(|score| score.action_id.clone())
                    .collect::<Vec<_>>()
            || selected_ids.len() + deferred_ids.len() + blocked_ids.len() != partition_union.len()
            || partition_union != score_ids
            || decision_order != self.mechanism_order
            || self
                .total_cost_units
                .saturating_add(self.budget_remaining_units)
                != self.budget_units
        {
            return Err(MechanismFeedbackReplanError::InvalidOutput(
                "action partitions, decision order, or budget accounting do not reconcile".into(),
            ));
        }
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| MechanismFeedbackReplanError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(MechanismFeedbackReplanError::Digest(
                "mechanism feedback replan digest is not content-addressed".into(),
            ));
        }
        Ok(())
    }
}

fn validate_request(
    request: &MechanismFeedbackReplanRequest,
) -> Result<(), MechanismFeedbackReplanError> {
    if request.objective.trim().is_empty()
        || request.prior_plan.objective != request.objective
        || request.candidates.is_empty()
        || request.candidates.len() > MAX_ACTIONS
        || request.observations.len() > MAX_OBSERVATIONS
        || request.max_actions == 0
        || request.max_actions > MAX_ACTIONS
        || request.budget_units == 0
        || request.minimum_priority_milli > 1_000
        || request.maximum_risk_milli > 1_000
    {
        return Err(MechanismFeedbackReplanError::InvalidRequest(
            "objective/plan binding, bounded candidates/observations/actions, budget, and thresholds are required".into(),
        ));
    }
    request
        .prior_plan
        .validate()
        .map_err(|error| MechanismFeedbackReplanError::InvalidRequest(error.to_string()))?;
    let mechanism_ids = request
        .prior_plan
        .mechanism_order
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let prior_actions = request
        .prior_plan
        .scores
        .iter()
        .map(|score| (score.action_id.clone(), score.mechanism_id.clone()))
        .collect::<BTreeMap<_, _>>();
    let mut action_ids = BTreeSet::new();
    for candidate in &request.candidates {
        if candidate.action_id.trim().is_empty()
            || !mechanism_ids.contains(&candidate.mechanism_id)
            || prior_actions.get(&candidate.action_id) != Some(&candidate.mechanism_id)
            || !action_ids.insert(candidate.action_id.clone())
            || candidate.risk_milli > 1_000
        {
            return Err(MechanismFeedbackReplanError::InvalidRequest(
                "candidate identity, prior-plan binding, mechanism coverage, risk bounds, and uniqueness are required".into(),
            ));
        }
    }
    let mut observation_ids = BTreeSet::new();
    let mut observed_actions = BTreeSet::new();
    for observation in &request.observations {
        if observation.observation_id.trim().is_empty()
            || !observation_ids.insert(observation.observation_id.clone())
            || !action_ids.contains(&observation.action_id)
            || prior_actions.get(&observation.action_id) != Some(&observation.mechanism_id)
            || observation.observed_information_gain_milli > 1_000
            || observation.uncertainty_milli > 1_000
            || !observed_actions.insert(observation.action_id.clone())
            || observation.artifact.validate().is_err()
        {
            return Err(MechanismFeedbackReplanError::InvalidRequest(
                "observation identity, action/mechanism binding, score bounds, uniqueness, and local artifact validity are required".into(),
            ));
        }
    }
    Ok(())
}

fn feedback_adjustment(
    outcome: Option<&MechanismFeedbackObservation>,
    prior_status: AssimilatedMechanismStatus,
) -> (i16, u16) {
    let Some(observation) = outcome else {
        return (
            match prior_status {
                AssimilatedMechanismStatus::Contradicted => 120,
                AssimilatedMechanismStatus::Unresolved => 80,
                AssimilatedMechanismStatus::Supported => 20,
            },
            0,
        );
    };
    let adjustment = match observation.outcome {
        MechanismFeedbackOutcome::Supported => -80,
        MechanismFeedbackOutcome::Contradicted => 220,
        MechanismFeedbackOutcome::Unresolved => 160,
        MechanismFeedbackOutcome::NotObserved => 0,
    };
    (adjustment, observation.observed_information_gain_milli)
}

/// Replan the next local mechanism batch from typed returned outcomes, preserving omissions and
/// contradictions rather than silently restarting from the previous frontier.
pub fn replan_glioma_mechanism_feedback(
    request: &MechanismFeedbackReplanRequest,
) -> Result<MechanismFeedbackReplan, MechanismFeedbackReplanError> {
    validate_request(request)?;
    let prior_scores = request
        .prior_plan
        .scores
        .iter()
        .map(|score| (score.action_id.clone(), score))
        .collect::<BTreeMap<_, _>>();
    let prior_decisions = request
        .prior_plan
        .decisions
        .iter()
        .map(|decision| (decision.mechanism_id.clone(), decision))
        .collect::<BTreeMap<_, _>>();
    let observations = request
        .observations
        .iter()
        .map(|observation| (observation.action_id.clone(), observation))
        .collect::<BTreeMap<_, _>>();
    let candidates = request
        .candidates
        .iter()
        .map(|candidate| (candidate.action_id.clone(), candidate))
        .collect::<BTreeMap<_, _>>();
    let mut scores = Vec::new();
    let mut uncertainty = request.prior_plan.uncertainty.clone();
    let mut negative_evidence = request.prior_plan.negative_evidence.clone();
    for candidate in &request.candidates {
        let prior_score = prior_scores
            .get(&candidate.action_id)
            .expect("validated prior action");
        let decision = prior_decisions
            .get(&candidate.mechanism_id)
            .expect("validated prior mechanism");
        let observation = observations.get(&candidate.action_id).copied();
        let (adjustment, observed_information_gain) =
            feedback_adjustment(observation, decision.status);
        let posterior_priority = (i32::from(prior_score.priority_milli)
            + i32::from(adjustment)
            + i32::from(observed_information_gain) / 4)
            .clamp(0, 1_000) as u16;
        if let Some(observation) = observation {
            match observation.outcome {
                MechanismFeedbackOutcome::Contradicted => negative_evidence.push(format!(
                    "{}:{}:contradicted",
                    candidate.mechanism_id, candidate.action_id
                )),
                MechanismFeedbackOutcome::NotObserved => uncertainty.push(format!(
                    "{}:{}:not-observed",
                    candidate.mechanism_id, candidate.action_id
                )),
                MechanismFeedbackOutcome::Unresolved => uncertainty.push(format!(
                    "{}:{}:unresolved",
                    candidate.mechanism_id, candidate.action_id
                )),
                MechanismFeedbackOutcome::Supported => {}
            }
        } else {
            uncertainty.push(format!(
                "{}:{}:no-feedback",
                candidate.mechanism_id, candidate.action_id
            ));
        }
        let exclusion_reason = if posterior_priority < request.minimum_priority_milli {
            Some("priority-below-threshold".into())
        } else if candidate.risk_milli > request.maximum_risk_milli {
            Some("risk-above-threshold".into())
        } else if candidate.requires_approval && !request.allow_approval_required {
            Some("approval-required-by-policy".into())
        } else {
            None
        };
        scores.push(MechanismFeedbackActionScore {
            action_id: candidate.action_id.clone(),
            mechanism_id: candidate.mechanism_id.clone(),
            prior_priority_milli: prior_score.priority_milli,
            feedback_adjustment_milli: adjustment,
            observed_information_gain_milli: observed_information_gain,
            posterior_priority_milli: posterior_priority,
            eligible: exclusion_reason.is_none(),
            exclusion_reason,
        });
    }
    scores.sort_by(|left, right| {
        right
            .posterior_priority_milli
            .cmp(&left.posterior_priority_milli)
            .then_with(|| left.action_id.cmp(&right.action_id))
    });
    let ranked_action_order = scores
        .iter()
        .map(|score| score.action_id.clone())
        .collect::<Vec<_>>();
    let candidate_order = {
        let mut values = ranked_action_order.clone();
        values.sort();
        values
    };
    let mut selected = Vec::new();
    let mut deferred = BTreeSet::new();
    let mut blocked = BTreeSet::new();
    let mut spent = 0_u64;
    for score in &scores {
        let candidate = candidates
            .get(&score.action_id)
            .expect("validated candidate");
        if !score.eligible {
            blocked.insert(score.action_id.clone());
        } else if selected.len() < request.max_actions
            && spent.saturating_add(u64::from(candidate.cost_units)) <= request.budget_units
        {
            spent = spent.saturating_add(u64::from(candidate.cost_units));
            selected.push(score.action_id.clone());
        } else {
            deferred.insert(score.action_id.clone());
        }
    }
    let mut decisions = Vec::new();
    for mechanism_id in &request.prior_plan.mechanism_order {
        let prior = prior_decisions
            .get(mechanism_id)
            .expect("validated decision");
        let outcomes = request
            .observations
            .iter()
            .filter(|observation| observation.mechanism_id == *mechanism_id)
            .map(|observation| observation.outcome)
            .collect::<Vec<_>>();
        let next_action_id = selected
            .iter()
            .find(|action_id| candidates[*action_id].mechanism_id == *mechanism_id)
            .cloned();
        let rationale = if outcomes
            .iter()
            .any(|outcome| *outcome == MechanismFeedbackOutcome::Contradicted)
        {
            "contradiction feedback increases pressure for a discriminating next action"
        } else if outcomes
            .iter()
            .any(|outcome| *outcome == MechanismFeedbackOutcome::Unresolved)
        {
            "unresolved feedback retains exploration pressure and explicit uncertainty"
        } else if outcomes
            .iter()
            .any(|outcome| *outcome == MechanismFeedbackOutcome::Supported)
        {
            "supportive feedback shifts the frontier toward bounded validation"
        } else if next_action_id.is_some() {
            "no returned outcome is available; prior uncertainty keeps a bounded action visible"
        } else {
            "no selected action remains within current policy bounds"
        };
        decisions.push(MechanismFeedbackDecision {
            mechanism_id: mechanism_id.clone(),
            prior_status: prior.status,
            outcome_order: outcomes,
            next_action_id,
            rationale: rationale.into(),
        });
    }
    uncertainty.sort();
    uncertainty.dedup();
    negative_evidence.sort();
    negative_evidence.dedup();
    let disposition = if selected.is_empty() && !request.candidates.is_empty() {
        MechanismFeedbackReplanDisposition::NoEligibleActions
    } else if selected.len() < request.candidates.len() || !uncertainty.is_empty() {
        MechanismFeedbackReplanDisposition::Partial
    } else {
        MechanismFeedbackReplanDisposition::Ready
    };
    let mut observation_order = request
        .observations
        .iter()
        .map(|observation| observation.observation_id.clone())
        .collect::<Vec<_>>();
    observation_order.sort();
    let mut output = MechanismFeedbackReplan {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        objective: request.objective.clone(),
        prior_plan_digest: request.prior_plan.digest.clone(),
        mechanism_order: request.prior_plan.mechanism_order.clone(),
        observation_order,
        decisions,
        candidate_order,
        ranked_action_order,
        selected_action_order: selected,
        deferred_action_order: deferred.into_iter().collect(),
        blocked_action_order: blocked.into_iter().collect(),
        scores,
        budget_units: request.budget_units,
        total_cost_units: spent,
        budget_remaining_units: request.budget_units.saturating_sub(spent),
        negative_evidence,
        uncertainty,
        disposition,
        next_route: "glioma_mechanism_action_plan".into(),
        digest: ContentHash::of_bytes(b"placeholder"),
    };
    output.digest = ContentHash::of_value(&digest_input(&output))
        .map_err(|error| MechanismFeedbackReplanError::Digest(error.to_string()))?;
    output.validate()?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::super::closed_loop::{
        digest_input, MechanismClosedLoopActionScore, MechanismClosedLoopDecision,
        MechanismClosedLoopDisposition,
    };
    use super::super::evidence_assimilation::AssimilatedMechanismStatus;
    use super::*;

    fn prior_plan() -> MechanismClosedLoopPlan {
        let mut plan = MechanismClosedLoopPlan {
            feature_id: super::super::closed_loop::FEATURE_ID.into(),
            output_schema: super::super::closed_loop::OUTPUT_SCHEMA.into(),
            objective: "feedback replan test".into(),
            assimilation_digest: ContentHash::of_bytes(b"assimilation"),
            mechanism_order: vec!["m1".into()],
            decisions: vec![MechanismClosedLoopDecision {
                mechanism_id: "m1".into(),
                status: AssimilatedMechanismStatus::Unresolved,
                posterior_milli: 500,
                trend_milli: 0,
                next_action_id: Some("action-a".into()),
                rationale: "uncertain".into(),
            }],
            candidate_order: vec!["action-a".into()],
            ranked_action_order: vec!["action-a".into()],
            selected_action_order: vec!["action-a".into()],
            deferred_action_order: vec![],
            blocked_action_order: vec![],
            scores: vec![MechanismClosedLoopActionScore {
                action_id: "action-a".into(),
                mechanism_id: "m1".into(),
                priority_milli: 600,
                information_gain_milli: 700,
                uncertainty_milli: 500,
                contradiction_pressure_milli: 850,
                trend_pressure_milli: 0,
                eligible: true,
                exclusion_reason: None,
            }],
            budget_units: 2,
            total_cost_units: 1,
            budget_remaining_units: 1,
            negative_evidence: vec!["prior:uncertain".into()],
            uncertainty: vec!["m1:prior".into()],
            disposition: MechanismClosedLoopDisposition::Partial,
            digest: ContentHash::of_bytes(b"placeholder"),
        };
        plan.digest = ContentHash::of_value(&digest_input(&plan)).unwrap();
        plan.validate().unwrap();
        plan
    }

    fn artifact(id: &str) -> LocalArtifactRef {
        LocalArtifactRef {
            artifact_id: id.into(),
            content_hash: ContentHash::of_bytes(id.as_bytes()),
            content_type: "application/vnd.aurora.glioma-observation+json".into(),
            local_only: true,
            contains_human_data: false,
            contains_direct_identifiers: false,
        }
    }

    #[test]
    fn contradiction_feedback_increases_priority_and_preserves_negative_evidence() {
        let output = replan_glioma_mechanism_feedback(&MechanismFeedbackReplanRequest {
            objective: "feedback replan test".into(),
            prior_plan: prior_plan(),
            candidates: vec![MechanismFeedbackCandidate {
                action_id: "action-a".into(),
                mechanism_id: "m1".into(),
                cost_units: 1,
                risk_milli: 100,
                requires_approval: false,
            }],
            observations: vec![MechanismFeedbackObservation {
                observation_id: "obs-a".into(),
                action_id: "action-a".into(),
                mechanism_id: "m1".into(),
                outcome: MechanismFeedbackOutcome::Contradicted,
                observed_information_gain_milli: 900,
                uncertainty_milli: 100,
                artifact: artifact("obs-a"),
            }],
            budget_units: 1,
            max_actions: 1,
            minimum_priority_milli: 500,
            maximum_risk_milli: 500,
            allow_approval_required: false,
        })
        .unwrap();
        assert_eq!(output.selected_action_order, vec!["action-a"]);
        assert_eq!(output.scores[0].feedback_adjustment_milli, 220);
        assert!(output
            .negative_evidence
            .iter()
            .any(|item| item.contains("contradicted")));
        output.validate().unwrap();
    }

    #[test]
    fn missing_feedback_remains_explicit_and_partial() {
        let output = replan_glioma_mechanism_feedback(&MechanismFeedbackReplanRequest {
            objective: "feedback replan test".into(),
            prior_plan: prior_plan(),
            candidates: vec![MechanismFeedbackCandidate {
                action_id: "action-a".into(),
                mechanism_id: "m1".into(),
                cost_units: 1,
                risk_milli: 100,
                requires_approval: false,
            }],
            observations: vec![],
            budget_units: 1,
            max_actions: 1,
            minimum_priority_milli: 0,
            maximum_risk_milli: 500,
            allow_approval_required: false,
        })
        .unwrap();
        assert_eq!(
            output.disposition,
            MechanismFeedbackReplanDisposition::Partial
        );
        assert!(output
            .uncertainty
            .iter()
            .any(|item| item.contains("no-feedback")));
    }
}
