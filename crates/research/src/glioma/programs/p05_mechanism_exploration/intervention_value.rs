//! Posterior-weighted intervention value for preclinical glioma mechanism research.
//!
//! This feature turns competing mechanistic predictions into a bounded assay portfolio.  It
//! prefers perturbations that separate plausible mechanisms while discounting uncertainty,
//! feasibility, cost, risk, and redundant targets.  The output is a research prioritisation
//! product only: it never prescribes treatment or dispatches a perturbation.

use crate::glioma_engine::GliomaModelSystem;
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P05-F04";
pub const OUTPUT_SCHEMA: &str = "GliomaMechanismInterventionValue1@1";
pub const MAX_CANDIDATES: usize = 2_048;
pub const MAX_MECHANISMS: usize = 128;
pub const MAX_SELECTED: usize = 256;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MechanismInterventionValueRequest {
    pub objective: String,
    pub model_system: GliomaModelSystem,
    pub budget_units: u64,
    pub max_selected: usize,
    pub min_value_milli: u64,
    pub min_disagreement_milli: u16,
    pub risk_ceiling_milli: u16,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MechanismInterventionPrediction {
    pub mechanism_id: String,
    pub prior_milli: u16,
    pub effect_milli: i64,
    pub uncertainty_milli: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MechanismInterventionCandidate {
    pub candidate_id: String,
    pub label: String,
    pub target_id: String,
    pub redundancy_group: String,
    pub predictions: Vec<MechanismInterventionPrediction>,
    pub feasibility_milli: u16,
    pub cost_units: u32,
    pub risk_milli: u16,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MechanismInterventionValueScore {
    pub candidate_id: String,
    pub target_id: String,
    pub label: String,
    pub expected_effect_milli: i64,
    pub pairwise_separation_milli: u64,
    pub disagreement_milli: u16,
    pub uncertainty_penalty_milli: u64,
    pub value_milli: u64,
    pub feasibility_milli: u16,
    pub cost_units: u32,
    pub risk_milli: u16,
    pub eligible: bool,
    pub exclusion_reason: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MechanismInterventionValueDisposition {
    Qualified,
    Partial,
    Unresolved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MechanismInterventionValue {
    pub feature_id: String,
    pub output_schema: String,
    pub objective: String,
    pub model_system: GliomaModelSystem,
    pub mechanism_order: Vec<String>,
    pub candidate_order: Vec<String>,
    pub ranked_candidate_order: Vec<String>,
    pub scores: Vec<MechanismInterventionValueScore>,
    pub selected_order: Vec<String>,
    pub deferred_order: Vec<String>,
    pub total_cost_units: u64,
    pub budget_remaining_units: u64,
    pub negative_evidence: Vec<String>,
    pub uncertainty: Vec<String>,
    pub disposition: MechanismInterventionValueDisposition,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum MechanismInterventionValueError {
    #[error("mechanism intervention value request is invalid: {0}")]
    InvalidRequest(String),
    #[error("mechanism intervention value input is invalid: {0}")]
    InvalidInput(String),
    #[error("mechanism intervention value output is invalid: {0}")]
    InvalidOutput(String),
    #[error("mechanism intervention value digest failed: {0}")]
    Digest(String),
}

fn digest_input(output: &MechanismInterventionValue) -> serde_json::Value {
    serde_json::json!({
        "feature_id": output.feature_id,
        "output_schema": output.output_schema,
        "objective": output.objective,
        "model_system": output.model_system,
        "mechanism_order": output.mechanism_order,
        "candidate_order": output.candidate_order,
        "ranked_candidate_order": output.ranked_candidate_order,
        "scores": output.scores,
        "selected_order": output.selected_order,
        "deferred_order": output.deferred_order,
        "total_cost_units": output.total_cost_units,
        "budget_remaining_units": output.budget_remaining_units,
        "negative_evidence": output.negative_evidence,
        "uncertainty": output.uncertainty,
        "disposition": output.disposition,
    })
}

fn canonical(values: &[String]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

impl MechanismInterventionValue {
    pub fn validate(&self) -> Result<(), MechanismInterventionValueError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || self.objective.trim().is_empty()
            || self.mechanism_order.len() < 2
            || !canonical(&self.mechanism_order)
            || !canonical(&self.candidate_order)
            || self
                .ranked_candidate_order
                .windows(2)
                .any(|pair| pair[0] == pair[1])
            || self.scores.len() != self.candidate_order.len()
            || !canonical(&self.deferred_order)
            || !canonical(&self.negative_evidence)
            || !canonical(&self.uncertainty)
            || self.scores.iter().any(|score| {
                score.candidate_id.trim().is_empty()
                    || score.target_id.trim().is_empty()
                    || score.label.trim().is_empty()
                    || score.feasibility_milli > 1_000
                    || score.disagreement_milli > 1_000
                    || score.risk_milli > 1_000
                    || (score.eligible && score.exclusion_reason.is_some())
                    || (!score.eligible && score.exclusion_reason.is_none())
            })
        {
            return Err(MechanismInterventionValueError::InvalidOutput(
                "identity, bounds, score cardinality, or canonical ordering is invalid".into(),
            ));
        }
        let candidate_ids = self
            .candidate_order
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>();
        let score_ids = self
            .scores
            .iter()
            .map(|score| score.candidate_id.clone())
            .collect::<BTreeSet<_>>();
        let ranked_ids = self
            .ranked_candidate_order
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>();
        let selected_ids = self.selected_order.iter().cloned().collect::<BTreeSet<_>>();
        let deferred_ids = self.deferred_order.iter().cloned().collect::<BTreeSet<_>>();
        if candidate_ids.len() != self.candidate_order.len()
            || candidate_ids != score_ids
            || ranked_ids != candidate_ids
            || selected_ids.intersection(&deferred_ids).next().is_some()
            || selected_ids
                .union(&deferred_ids)
                .any(|id| !candidate_ids.contains(id))
        {
            return Err(MechanismInterventionValueError::InvalidOutput(
                "candidate, ranking, selected, or deferred partitions do not reconcile".into(),
            ));
        }
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| MechanismInterventionValueError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(MechanismInterventionValueError::InvalidOutput(
                "digest is not bound to intervention value output".into(),
            ));
        }
        Ok(())
    }
}

fn pairwise_separation(predictions: &[MechanismInterventionPrediction]) -> u64 {
    let mut total = 0_u128;
    for (left_index, left) in predictions.iter().enumerate() {
        for right in predictions.iter().skip(left_index + 1) {
            let prior = u128::from(left.prior_milli) * u128::from(right.prior_milli);
            total = total.saturating_add(
                prior.saturating_mul(left.effect_milli.abs_diff(right.effect_milli) as u128),
            );
        }
    }
    (total / 1_000_000).min(u128::from(u64::MAX)) as u64
}

fn expected_effect(predictions: &[MechanismInterventionPrediction]) -> i64 {
    let total = predictions
        .iter()
        .map(|prediction| i128::from(prediction.prior_milli) * i128::from(prediction.effect_milli))
        .sum::<i128>();
    (total / 1_000).clamp(i128::from(i64::MIN), i128::from(i64::MAX)) as i64
}

fn weighted_uncertainty(predictions: &[MechanismInterventionPrediction]) -> u64 {
    let total = predictions
        .iter()
        .map(|prediction| {
            u128::from(prediction.prior_milli) * u128::from(prediction.uncertainty_milli)
        })
        .sum::<u128>();
    (total / 1_000).min(u128::from(u64::MAX)) as u64
}

fn score_candidate(candidate: &MechanismInterventionCandidate) -> MechanismInterventionValueScore {
    let separation = pairwise_separation(&candidate.predictions);
    let uncertainty = weighted_uncertainty(&candidate.predictions);
    let expected = expected_effect(&candidate.predictions);
    let disagreement = separation.min(1_000) as u16;
    let feasibility = u64::from(candidate.feasibility_milli);
    let risk = u64::from(candidate.risk_milli);
    let cost = u64::from(candidate.cost_units.max(1));
    let value = separation
        .saturating_sub(uncertainty)
        .saturating_mul(feasibility)
        .saturating_mul(1_000_u64.saturating_sub(risk))
        / (cost.saturating_mul(1_000));
    let eligible = candidate.cost_units > 0
        && candidate.feasibility_milli > 0
        && candidate.risk_milli <= 1_000;
    MechanismInterventionValueScore {
        candidate_id: candidate.candidate_id.clone(),
        target_id: candidate.target_id.clone(),
        label: candidate.label.clone(),
        expected_effect_milli: expected,
        pairwise_separation_milli: separation,
        disagreement_milli: disagreement,
        uncertainty_penalty_milli: uncertainty,
        value_milli: value,
        feasibility_milli: candidate.feasibility_milli,
        cost_units: candidate.cost_units,
        risk_milli: candidate.risk_milli,
        eligible,
        exclusion_reason: if eligible {
            None
        } else {
            Some("invalid_cost_feasibility_or_risk".into())
        },
    }
}

pub fn analyze_glioma_mechanism_intervention_value(
    request: &MechanismInterventionValueRequest,
    candidates: &[MechanismInterventionCandidate],
) -> Result<MechanismInterventionValue, MechanismInterventionValueError> {
    if request.objective.trim().is_empty()
        || request.budget_units == 0
        || request.max_selected == 0
        || request.max_selected > MAX_SELECTED
        || request.min_disagreement_milli > 1_000
        || request.risk_ceiling_milli > 1_000
    {
        return Err(MechanismInterventionValueError::InvalidRequest(
            "objective, budget, selection limit, disagreement, and risk bounds are invalid".into(),
        ));
    }
    if candidates.is_empty() || candidates.len() > MAX_CANDIDATES {
        return Err(MechanismInterventionValueError::InvalidInput(
            "candidate count is outside the supported bound".into(),
        ));
    }
    let mut candidate_ids = BTreeSet::new();
    let mut mechanism_order = BTreeSet::new();
    let mut expected_mechanisms: Option<BTreeSet<String>> = None;
    let mut scores = Vec::with_capacity(candidates.len());
    for candidate in candidates {
        if !candidate_ids.insert(candidate.candidate_id.clone())
            || candidate.candidate_id.trim().is_empty()
            || candidate.label.trim().is_empty()
            || candidate.target_id.trim().is_empty()
            || candidate.redundancy_group.trim().is_empty()
            || candidate.predictions.len() < 2
            || candidate.predictions.len() > MAX_MECHANISMS
            || candidate.feasibility_milli > 1_000
            || candidate.risk_milli > 1_000
            || candidate.predictions.iter().any(|prediction| {
                prediction.mechanism_id.trim().is_empty()
                    || prediction.prior_milli == 0
                    || prediction.uncertainty_milli == 0
            })
        {
            return Err(MechanismInterventionValueError::InvalidInput(
                "candidate identity, predictions, priors, uncertainty, or bounds are invalid"
                    .into(),
            ));
        }
        let local_ids = candidate
            .predictions
            .iter()
            .map(|prediction| prediction.mechanism_id.clone())
            .collect::<BTreeSet<_>>();
        let prior_sum = candidate
            .predictions
            .iter()
            .map(|prediction| u32::from(prediction.prior_milli))
            .sum::<u32>();
        if prior_sum != 1_000 || local_ids.len() != candidate.predictions.len() {
            return Err(MechanismInterventionValueError::InvalidInput(
                "each candidate must cover unique mechanisms with priors summing to 1000".into(),
            ));
        }
        if let Some(expected) = &expected_mechanisms {
            if expected != &local_ids {
                return Err(MechanismInterventionValueError::InvalidInput(
                    "all candidates must cover the same mechanism ensemble".into(),
                ));
            }
        } else {
            expected_mechanisms = Some(local_ids.clone());
        }
        mechanism_order.extend(local_ids);
        scores.push(score_candidate(candidate));
    }
    if mechanism_order.len() < 2 || mechanism_order.len() > MAX_MECHANISMS {
        return Err(MechanismInterventionValueError::InvalidInput(
            "mechanism ensemble is outside the supported bound".into(),
        ));
    }
    let mechanism_order = mechanism_order.into_iter().collect::<Vec<_>>();
    let candidate_order = candidate_ids.iter().cloned().collect::<Vec<_>>();
    let mut ranked = scores.clone();
    ranked.sort_by(|left, right| {
        right
            .value_milli
            .cmp(&left.value_milli)
            .then_with(|| right.disagreement_milli.cmp(&left.disagreement_milli))
            .then_with(|| left.candidate_id.cmp(&right.candidate_id))
    });
    let ranked_candidate_order = ranked
        .iter()
        .map(|score| score.candidate_id.clone())
        .collect::<Vec<_>>();
    let mut selected_order = Vec::new();
    let mut deferred_order = Vec::new();
    let mut groups = BTreeSet::new();
    let candidate_by_id = candidates
        .iter()
        .map(|candidate| (candidate.candidate_id.as_str(), candidate))
        .collect::<BTreeMap<_, _>>();
    let mut total_cost_units = 0_u64;
    for score in &ranked {
        let candidate = candidate_by_id[score.candidate_id.as_str()];
        let selectable = score.eligible
            && score.value_milli >= request.min_value_milli
            && score.disagreement_milli >= request.min_disagreement_milli
            && score.risk_milli <= request.risk_ceiling_milli
            && !groups.contains(&candidate.redundancy_group)
            && selected_order.len() < request.max_selected
            && total_cost_units.saturating_add(u64::from(score.cost_units)) <= request.budget_units;
        if selectable {
            selected_order.push(score.candidate_id.clone());
            groups.insert(candidate.redundancy_group.clone());
            total_cost_units = total_cost_units.saturating_add(u64::from(score.cost_units));
        } else {
            deferred_order.push(score.candidate_id.clone());
        }
    }
    let mut negative_evidence = Vec::new();
    let mut uncertainty = Vec::new();
    if selected_order.is_empty() {
        negative_evidence.push(
            "no candidate survived value, disagreement, risk, redundancy, and budget gates".into(),
        );
    }
    if scores
        .iter()
        .any(|score| score.uncertainty_penalty_milli > score.pairwise_separation_milli)
    {
        uncertainty
            .push("prediction uncertainty exceeds separation for at least one candidate".into());
    }
    if candidates.iter().any(|candidate| {
        candidate
            .predictions
            .iter()
            .any(|prediction| prediction.uncertainty_milli > 500_000)
    }) {
        uncertainty.push("at least one mechanism prediction has high declared uncertainty".into());
    }
    negative_evidence.sort();
    uncertainty.sort();
    let disposition = if selected_order.is_empty() {
        MechanismInterventionValueDisposition::Unresolved
    } else if selected_order.len() < request.max_selected || !deferred_order.is_empty() {
        MechanismInterventionValueDisposition::Partial
    } else {
        MechanismInterventionValueDisposition::Qualified
    };
    let mut output = MechanismInterventionValue {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        objective: request.objective.clone(),
        model_system: request.model_system,
        mechanism_order,
        candidate_order,
        ranked_candidate_order,
        scores,
        selected_order,
        deferred_order,
        total_cost_units,
        budget_remaining_units: request.budget_units.saturating_sub(total_cost_units),
        negative_evidence,
        uncertainty,
        disposition,
        digest: ContentHash::of_value(&serde_json::Value::Null)
            .map_err(|error| MechanismInterventionValueError::Digest(error.to_string()))?,
    };
    output.digest = ContentHash::of_value(&digest_input(&output))
        .map_err(|error| MechanismInterventionValueError::Digest(error.to_string()))?;
    output.validate()?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request() -> MechanismInterventionValueRequest {
        MechanismInterventionValueRequest {
            objective: "separate invasion mechanisms".into(),
            model_system: GliomaModelSystem::Organoid,
            budget_units: 10,
            max_selected: 2,
            min_value_milli: 1,
            min_disagreement_milli: 1,
            risk_ceiling_milli: 600,
        }
    }

    fn candidate(id: &str, group: &str, left: i64, right: i64) -> MechanismInterventionCandidate {
        MechanismInterventionCandidate {
            candidate_id: id.into(),
            label: id.into(),
            target_id: format!("target-{id}"),
            redundancy_group: group.into(),
            predictions: vec![
                MechanismInterventionPrediction {
                    mechanism_id: "m1".into(),
                    prior_milli: 600,
                    effect_milli: left,
                    uncertainty_milli: 20,
                },
                MechanismInterventionPrediction {
                    mechanism_id: "m2".into(),
                    prior_milli: 400,
                    effect_milli: right,
                    uncertainty_milli: 20,
                },
            ],
            feasibility_milli: 900,
            cost_units: 2,
            risk_milli: 200,
        }
    }

    #[test]
    fn selects_high_separation_and_preserves_negative_evidence() {
        let output = analyze_glioma_mechanism_intervention_value(
            &request(),
            &[
                candidate("high", "g1", 900, -900),
                candidate("low", "g2", 30, 20),
            ],
        )
        .expect("valid intervention value");
        assert_eq!(output.selected_order, vec!["high"]);
        assert!(output.deferred_order.contains(&"low".to_string()));
        output.validate().expect("digest and partitions validate");
    }

    #[test]
    fn redundancy_and_risk_gates_remain_explicit() {
        let mut unsafe_candidate = candidate("unsafe", "g1", 900, -900);
        unsafe_candidate.risk_milli = 900;
        let output = analyze_glioma_mechanism_intervention_value(
            &request(),
            &[candidate("a", "g1", 800, -800), unsafe_candidate],
        )
        .expect("valid intervention value");
        assert_eq!(output.selected_order, vec!["a"]);
        assert!(output.deferred_order.contains(&"unsafe".to_string()));
    }
}
