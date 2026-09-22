//! Closed-loop mechanism evidence control for preclinical glioma research.
//!
//! This feature consumes an assimilated mechanism ledger and scores the next local research
//! actions against posterior uncertainty, contradiction, trend, information gain, safety, and
//! reproducibility.  It is a bounded controller for the next research round: it does not execute
//! assays, promote simulated outcomes to observations, or make clinical decisions.

use super::evidence_assimilation::{AssimilatedMechanismStatus, MechanismEvidenceAssimilation};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P05-F23";
pub const OUTPUT_SCHEMA: &str = "GliomaMechanismClosedLoop1@1";
pub const MAX_CANDIDATES: usize = 1_024;
pub const MAX_SELECTED: usize = 256;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MechanismClosedLoopCandidate {
    pub action_id: String,
    pub mechanism_id: String,
    pub expected_information_gain_milli: u16,
    pub expected_effect_milli: i64,
    pub cost_units: u32,
    pub risk_milli: u16,
    pub reproducibility_milli: u16,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MechanismClosedLoopRequest {
    pub objective: String,
    pub assimilation: MechanismEvidenceAssimilation,
    pub candidates: Vec<MechanismClosedLoopCandidate>,
    pub budget_units: u64,
    pub max_actions: usize,
    pub minimum_information_gain_milli: u16,
    pub maximum_risk_milli: u16,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MechanismClosedLoopActionScore {
    pub action_id: String,
    pub mechanism_id: String,
    pub priority_milli: u16,
    pub information_gain_milli: u16,
    pub uncertainty_milli: u16,
    pub contradiction_pressure_milli: u16,
    pub trend_pressure_milli: u16,
    pub eligible: bool,
    pub exclusion_reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MechanismClosedLoopDecision {
    pub mechanism_id: String,
    pub status: AssimilatedMechanismStatus,
    pub posterior_milli: u16,
    pub trend_milli: i32,
    pub next_action_id: Option<String>,
    pub rationale: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MechanismClosedLoopDisposition {
    Ready,
    Partial,
    Blocked,
    NoEligibleActions,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MechanismClosedLoopPlan {
    pub feature_id: String,
    pub output_schema: String,
    pub objective: String,
    pub assimilation_digest: ContentHash,
    pub mechanism_order: Vec<String>,
    pub decisions: Vec<MechanismClosedLoopDecision>,
    pub candidate_order: Vec<String>,
    pub ranked_action_order: Vec<String>,
    pub selected_action_order: Vec<String>,
    pub deferred_action_order: Vec<String>,
    pub blocked_action_order: Vec<String>,
    pub scores: Vec<MechanismClosedLoopActionScore>,
    pub budget_units: u64,
    pub total_cost_units: u64,
    pub budget_remaining_units: u64,
    pub negative_evidence: Vec<String>,
    pub uncertainty: Vec<String>,
    pub disposition: MechanismClosedLoopDisposition,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum MechanismClosedLoopError {
    #[error("mechanism closed-loop request is invalid: {0}")]
    InvalidRequest(String),
    #[error("mechanism closed-loop output is invalid: {0}")]
    InvalidOutput(String),
    #[error("mechanism closed-loop digest failed: {0}")]
    Digest(String),
}

fn canonical(values: &[String]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn unique_nonempty(values: &[String]) -> bool {
    values.iter().all(|value| !value.trim().is_empty())
        && values.iter().collect::<BTreeSet<_>>().len() == values.len()
}

pub(crate) fn digest_input(plan: &MechanismClosedLoopPlan) -> serde_json::Value {
    serde_json::json!({
        "feature_id": plan.feature_id,
        "output_schema": plan.output_schema,
        "objective": plan.objective,
        "assimilation_digest": plan.assimilation_digest,
        "mechanism_order": plan.mechanism_order,
        "decisions": plan.decisions,
        "candidate_order": plan.candidate_order,
        "ranked_action_order": plan.ranked_action_order,
        "selected_action_order": plan.selected_action_order,
        "deferred_action_order": plan.deferred_action_order,
        "blocked_action_order": plan.blocked_action_order,
        "scores": plan.scores,
        "budget_units": plan.budget_units,
        "total_cost_units": plan.total_cost_units,
        "budget_remaining_units": plan.budget_remaining_units,
        "negative_evidence": plan.negative_evidence,
        "uncertainty": plan.uncertainty,
        "disposition": plan.disposition,
    })
}

impl MechanismClosedLoopPlan {
    pub fn validate(&self) -> Result<(), MechanismClosedLoopError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || self.objective.trim().is_empty()
            || self.assimilation_digest.as_str().len() != 64
            || !canonical(&self.mechanism_order)
            || !unique_nonempty(&self.mechanism_order)
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
            || self.scores.iter().any(|score| {
                score.action_id.trim().is_empty()
                    || score.mechanism_id.trim().is_empty()
                    || score.priority_milli > 1_000
                    || score.information_gain_milli > 1_000
                    || score.uncertainty_milli > 1_000
                    || score.contradiction_pressure_milli > 1_000
                    || score.trend_pressure_milli > 1_000
            })
        {
            return Err(MechanismClosedLoopError::InvalidOutput(
                "identity, ordering, score bounds, or digest shape is invalid".into(),
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
            return Err(MechanismClosedLoopError::InvalidOutput(
                "candidate partitions or budget accounting do not reconcile".into(),
            ));
        }
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| MechanismClosedLoopError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(MechanismClosedLoopError::Digest(
                "mechanism closed-loop digest is not content-addressed".into(),
            ));
        }
        Ok(())
    }
}

fn validate_request(request: &MechanismClosedLoopRequest) -> Result<(), MechanismClosedLoopError> {
    if request.objective.trim().is_empty()
        || request.assimilation.objective != request.objective
        || request.candidates.len() > MAX_CANDIDATES
        || request.max_actions == 0
        || request.max_actions > MAX_SELECTED
        || request.budget_units == 0
        || request.minimum_information_gain_milli > 1_000
        || request.maximum_risk_milli > 1_000
    {
        return Err(MechanismClosedLoopError::InvalidRequest(
            "objective/assimilation binding, bounded candidates/actions, budget, and thresholds are required".into(),
        ));
    }
    request
        .assimilation
        .validate()
        .map_err(|error| MechanismClosedLoopError::InvalidRequest(error.to_string()))?;
    let mechanism_ids = request
        .assimilation
        .mechanism_order
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let mut action_ids = BTreeSet::new();
    for candidate in &request.candidates {
        if candidate.action_id.trim().is_empty()
            || !mechanism_ids.contains(&candidate.mechanism_id)
            || !action_ids.insert(candidate.action_id.clone())
            || candidate.expected_information_gain_milli > 1_000
            || candidate.risk_milli > 1_000
            || candidate.reproducibility_milli > 1_000
        {
            return Err(MechanismClosedLoopError::InvalidRequest(
                "candidate identity, known mechanism, score bounds, and uniqueness are required"
                    .into(),
            ));
        }
    }
    Ok(())
}

fn priority_score(
    candidate: &MechanismClosedLoopCandidate,
    record: &super::evidence_assimilation::AssimilatedMechanismRecord,
) -> (u16, u16, u16, u16, u16) {
    let uncertainty = 1_000_u16.saturating_sub(record.posterior_milli);
    let contradiction = match record.status {
        AssimilatedMechanismStatus::Contradicted => 1_000,
        AssimilatedMechanismStatus::Unresolved => 850,
        AssimilatedMechanismStatus::Supported => 250,
    };
    let trend = record.trend_milli.unsigned_abs().min(1_000) as u16;
    let raw = (u32::from(candidate.expected_information_gain_milli) * 40
        + u32::from(uncertainty) * 25
        + u32::from(contradiction) * 15
        + u32::from(candidate.reproducibility_milli) * 15
        + u32::from(trend) * 5)
        / 100;
    let penalty = (u32::from(candidate.risk_milli) * 20
        + u32::try_from(candidate.cost_units.min(1_000)).unwrap_or(1_000) * 5)
        / 100;
    let score = raw.saturating_sub(penalty).min(1_000) as u16;
    (
        score,
        candidate.expected_information_gain_milli,
        uncertainty,
        contradiction,
        trend,
    )
}

/// Assimilate a mechanism ledger into a bounded next-action control frontier.
pub fn plan_glioma_mechanism_closed_loop(
    request: &MechanismClosedLoopRequest,
) -> Result<MechanismClosedLoopPlan, MechanismClosedLoopError> {
    validate_request(request)?;
    let records = request
        .assimilation
        .records
        .iter()
        .map(|record| (record.mechanism_id.clone(), record))
        .collect::<BTreeMap<_, _>>();
    let mut scores = Vec::new();
    for candidate in &request.candidates {
        let record = records
            .get(&candidate.mechanism_id)
            .expect("validated mechanism");
        let (priority, _gain, uncertainty, contradiction, trend) =
            priority_score(candidate, record);
        let exclusion_reason =
            if candidate.expected_information_gain_milli < request.minimum_information_gain_milli {
                Some("information-gain-below-threshold".into())
            } else if candidate.risk_milli > request.maximum_risk_milli {
                Some("risk-above-threshold".into())
            } else {
                None
            };
        scores.push(MechanismClosedLoopActionScore {
            action_id: candidate.action_id.clone(),
            mechanism_id: candidate.mechanism_id.clone(),
            priority_milli: priority,
            information_gain_milli: candidate.expected_information_gain_milli,
            uncertainty_milli: uncertainty,
            contradiction_pressure_milli: contradiction,
            trend_pressure_milli: trend,
            eligible: exclusion_reason.is_none(),
            exclusion_reason,
        });
    }
    scores.sort_by(|left, right| {
        right
            .priority_milli
            .cmp(&left.priority_milli)
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
    let candidate_map = request
        .candidates
        .iter()
        .map(|candidate| (candidate.action_id.clone(), candidate))
        .collect::<BTreeMap<_, _>>();
    let mut selected = Vec::new();
    let mut deferred = BTreeSet::new();
    let mut blocked = BTreeSet::new();
    let mut spent = 0_u64;
    for score in &scores {
        let candidate = candidate_map
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
    let mut uncertainty = request.assimilation.uncertainty.clone();
    for mechanism_id in &request.assimilation.mechanism_order {
        let record = records.get(mechanism_id).expect("validated record");
        let next_action_id = selected
            .iter()
            .find(|action_id| candidate_map[*action_id].mechanism_id == *mechanism_id)
            .cloned();
        let rationale = match (record.status, next_action_id.is_some()) {
            (AssimilatedMechanismStatus::Contradicted, true) => {
                "contradiction pressure selects a local discriminating action".into()
            }
            (AssimilatedMechanismStatus::Unresolved, true) => {
                "uncertainty selects a local information-gain action".into()
            }
            (AssimilatedMechanismStatus::Supported, true) => {
                "supported mechanism receives a bounded validation action".into()
            }
            (_, false) => {
                uncertainty.push(format!("{mechanism_id}:no-selected-action"));
                "no eligible budgeted action selected; mechanism remains explicit".into()
            }
        };
        decisions.push(MechanismClosedLoopDecision {
            mechanism_id: mechanism_id.clone(),
            status: record.status,
            posterior_milli: record.posterior_milli,
            trend_milli: record.trend_milli,
            next_action_id,
            rationale,
        });
    }
    uncertainty.sort();
    uncertainty.dedup();
    let disposition = if request.assimilation.disposition
        == super::evidence_assimilation::MechanismEvidenceAssimilationDisposition::Blocked
    {
        MechanismClosedLoopDisposition::Blocked
    } else if selected.is_empty() && !request.candidates.is_empty() {
        MechanismClosedLoopDisposition::NoEligibleActions
    } else if selected.len() < request.candidates.len() || !uncertainty.is_empty() {
        MechanismClosedLoopDisposition::Partial
    } else {
        MechanismClosedLoopDisposition::Ready
    };
    let mut output = MechanismClosedLoopPlan {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        objective: request.objective.clone(),
        assimilation_digest: request.assimilation.digest.clone(),
        mechanism_order: request.assimilation.mechanism_order.clone(),
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
        negative_evidence: request.assimilation.negative_evidence.clone(),
        uncertainty,
        disposition,
        digest: ContentHash::of_bytes(b"placeholder"),
    };
    output.digest = ContentHash::of_value(&digest_input(&output))
        .map_err(|error| MechanismClosedLoopError::Digest(error.to_string()))?;
    output.validate()?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::super::bayesian_update::{
        update_glioma_mechanism_posterior, BayesianMechanismHypothesis,
        BayesianMechanismUpdateRequest,
    };
    use super::super::discrimination::{MechanismFeatureObservation, MechanismPrediction};
    use super::super::evidence_assimilation::{
        assimilate_glioma_mechanism_evidence, MechanismEvidenceAssimilationRequest,
        MechanismEvidenceSnapshot,
    };
    use super::*;
    use crate::glioma_engine::{GliomaModelSystem, LocalArtifactRef};

    fn update(value: i64, label: &str) -> super::super::bayesian_update::MechanismBayesianUpdate {
        let observation = MechanismFeatureObservation {
            feature_id: "growth".into(),
            observed_milli: value,
            uncertainty_milli: 10,
            artifact: LocalArtifactRef {
                artifact_id: format!("artifact-{label}"),
                content_hash: ContentHash::of_bytes(label.as_bytes()),
                content_type: "tabular-feature".into(),
                local_only: true,
                contains_human_data: false,
                contains_direct_identifiers: false,
            },
        };
        update_glioma_mechanism_posterior(
            &BayesianMechanismUpdateRequest {
                objective: "closed-loop mechanism evidence".into(),
                model_system: GliomaModelSystem::Organoid,
                min_shared_features: 1,
                max_hypotheses: 4,
                likelihood_scale_milli: 100,
                supported_posterior_floor_milli: 600,
                contradicted_posterior_ceiling_milli: 100,
            },
            &[
                BayesianMechanismHypothesis {
                    mechanism_id: "near".into(),
                    statement: "near mechanism".into(),
                    prior_milli: 500,
                    predictions: vec![MechanismPrediction {
                        feature_id: "growth".into(),
                        predicted_milli: 100,
                        uncertainty_milli: 10,
                    }],
                },
                BayesianMechanismHypothesis {
                    mechanism_id: "far".into(),
                    statement: "far mechanism".into(),
                    prior_milli: 500,
                    predictions: vec![MechanismPrediction {
                        feature_id: "growth".into(),
                        predicted_milli: 900,
                        uncertainty_milli: 10,
                    }],
                },
            ],
            &[observation],
        )
        .unwrap()
    }

    fn request() -> MechanismClosedLoopRequest {
        let assimilation =
            assimilate_glioma_mechanism_evidence(&MechanismEvidenceAssimilationRequest {
                objective: "closed-loop mechanism evidence".into(),
                snapshots: vec![MechanismEvidenceSnapshot {
                    epoch_id: "epoch-1".into(),
                    update: update(110, "first"),
                }],
                recency_decay_milli: 800,
                max_snapshots: 8,
                supported_floor_milli: 600,
                contradicted_ceiling_milli: 100,
                preserve_negative_results: true,
            })
            .unwrap();
        MechanismClosedLoopRequest {
            objective: "closed-loop mechanism evidence".into(),
            assimilation,
            candidates: vec![
                MechanismClosedLoopCandidate {
                    action_id: "action-near".into(),
                    mechanism_id: "near".into(),
                    expected_information_gain_milli: 850,
                    expected_effect_milli: 50,
                    cost_units: 2,
                    risk_milli: 100,
                    reproducibility_milli: 900,
                },
                MechanismClosedLoopCandidate {
                    action_id: "action-far".into(),
                    mechanism_id: "far".into(),
                    expected_information_gain_milli: 900,
                    expected_effect_milli: -20,
                    cost_units: 2,
                    risk_milli: 100,
                    reproducibility_milli: 850,
                },
            ],
            budget_units: 4,
            max_actions: 2,
            minimum_information_gain_milli: 500,
            maximum_risk_milli: 500,
        }
    }

    #[test]
    fn closed_loop_selects_bounded_actions_and_preserves_digest() {
        let plan = plan_glioma_mechanism_closed_loop(&request()).unwrap();
        assert_eq!(plan.selected_action_order.len(), 2);
        assert_eq!(plan.total_cost_units, 4);
        assert!(plan.decisions.iter().any(|decision| {
            decision.mechanism_id == "far"
                && decision.status == AssimilatedMechanismStatus::Unresolved
        }));
        assert!(
            plan.uncertainty.is_empty() || plan.uncertainty.iter().any(|item| item.contains(':'))
        );
        plan.validate().unwrap();
    }

    #[test]
    fn closed_loop_blocks_actions_below_information_gate() {
        let mut request = request();
        request.minimum_information_gain_milli = 1_000;
        let plan = plan_glioma_mechanism_closed_loop(&request).unwrap();
        assert_eq!(
            plan.disposition,
            MechanismClosedLoopDisposition::NoEligibleActions
        );
        assert!(plan.selected_action_order.is_empty());
        assert_eq!(plan.blocked_action_order.len(), 2);
        plan.validate().unwrap();
    }
}
