//! Bounded value-of-information portfolio optimization for preclinical glioma decisions.
//!
//! The optimizer searches combinations of typed next actions rather than selecting a single
//! highest-scoring row. It rewards expected information gain, uncertainty and contradiction
//! reduction, reproducibility, and cross-modality/model diversity while retaining dependency,
//! budget, availability, and bounded-search constraints. Forecast utility remains a planning
//! value; no candidate is treated as an observed scientific result.

use crate::glioma_engine::{GliomaModality, GliomaModelSystem};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P04-F13";
pub const OUTPUT_SCHEMA: &str = "GliomaDecisionValueOptimizer1@1";
pub const MAX_CANDIDATES: usize = 256;
pub const MAX_ALTERNATIVES: usize = 16;
pub const MAX_BEAM_WIDTH: usize = 512;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DecisionValueCandidate {
    pub action_id: String,
    pub claim_id: String,
    pub modality: GliomaModality,
    pub model_system: GliomaModelSystem,
    pub depends_on: Vec<String>,
    pub cost_units: u32,
    pub information_gain_milli: u16,
    pub uncertainty_reduction_milli: u16,
    pub contradiction_resolution_milli: u16,
    pub reproducibility_milli: u16,
    pub failure_probability_milli: u16,
    pub diversity_group: String,
    pub available: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DecisionValueWeights {
    pub information_gain: u16,
    pub uncertainty_reduction: u16,
    pub contradiction_resolution: u16,
    pub reproducibility: u16,
    pub diversity: u16,
    pub failure_penalty: u16,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DecisionValueRequest {
    pub objective: String,
    pub candidates: Vec<DecisionValueCandidate>,
    pub budget_units: u32,
    pub max_actions: u16,
    pub beam_width: u16,
    pub max_alternatives: u16,
    pub require_dependency_closure: bool,
    pub min_candidate_utility_milli: i32,
    pub weights: DecisionValueWeights,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DecisionValueDisposition {
    Selected,
    Deferred,
    Blocked,
    Unresolved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DecisionValueCandidateScore {
    pub action_id: String,
    pub intrinsic_utility_milli: i32,
    pub marginal_utility_milli: i32,
    pub disposition: DecisionValueDisposition,
    pub reason_order: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DecisionValuePortfolio {
    pub portfolio_id: String,
    pub action_order: Vec<String>,
    pub cost_units: u32,
    pub utility_milli: i32,
    pub diversity_milli: u16,
    pub modality_order: Vec<GliomaModality>,
    pub model_system_order: Vec<GliomaModelSystem>,
    pub group_order: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DecisionValueCampaignDisposition {
    Ready,
    Partial,
    BudgetLimited,
    Unresolved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DecisionValueResult {
    pub feature_id: String,
    pub output_schema: String,
    pub objective: String,
    pub candidate_order: Vec<String>,
    pub selected_order: Vec<String>,
    pub deferred_order: Vec<String>,
    pub blocked_order: Vec<String>,
    pub selected_portfolio: Option<DecisionValuePortfolio>,
    pub alternatives: Vec<DecisionValuePortfolio>,
    pub candidate_scores: Vec<DecisionValueCandidateScore>,
    pub negative_evidence_order: Vec<String>,
    pub uncertainty_order: Vec<String>,
    pub disposition: DecisionValueCampaignDisposition,
    pub next_action: String,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum DecisionValueError {
    #[error("decision value request is invalid: {0}")]
    InvalidRequest(String),
    #[error("decision value candidate is invalid: {0}")]
    InvalidCandidate(String),
    #[error("decision value output is invalid: {0}")]
    InvalidOutput(String),
    #[error("decision value digest failed: {0}")]
    Digest(String),
}

fn canonical<T: Ord>(values: &[T]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn weighted_utility(candidate: &DecisionValueCandidate, weights: &DecisionValueWeights) -> i32 {
    let positive = i64::from(candidate.information_gain_milli)
        .saturating_mul(i64::from(weights.information_gain))
        .saturating_add(
            i64::from(candidate.uncertainty_reduction_milli)
                .saturating_mul(i64::from(weights.uncertainty_reduction)),
        )
        .saturating_add(
            i64::from(candidate.contradiction_resolution_milli)
                .saturating_mul(i64::from(weights.contradiction_resolution)),
        )
        .saturating_add(
            i64::from(candidate.reproducibility_milli)
                .saturating_mul(i64::from(weights.reproducibility)),
        );
    let failure = i64::from(candidate.failure_probability_milli)
        .saturating_mul(i64::from(weights.failure_penalty));
    ((positive.saturating_sub(failure)) / 100).clamp(i64::from(i32::MIN), i64::from(i32::MAX))
        as i32
}

fn digest_input(result: &DecisionValueResult) -> serde_json::Value {
    serde_json::json!({
        "feature_id": result.feature_id,
        "output_schema": result.output_schema,
        "objective": result.objective,
        "candidate_order": result.candidate_order,
        "selected_order": result.selected_order,
        "deferred_order": result.deferred_order,
        "blocked_order": result.blocked_order,
        "selected_portfolio": result.selected_portfolio,
        "alternatives": result.alternatives,
        "candidate_scores": result.candidate_scores,
        "negative_evidence_order": result.negative_evidence_order,
        "uncertainty_order": result.uncertainty_order,
        "disposition": result.disposition,
        "next_action": result.next_action,
    })
}

fn validate_request(request: &DecisionValueRequest) -> Result<(), DecisionValueError> {
    let weights_sum = u32::from(request.weights.information_gain)
        + u32::from(request.weights.uncertainty_reduction)
        + u32::from(request.weights.contradiction_resolution)
        + u32::from(request.weights.reproducibility)
        + u32::from(request.weights.diversity)
        + u32::from(request.weights.failure_penalty);
    if request.objective.trim().is_empty()
        || request.candidates.is_empty()
        || request.candidates.len() > MAX_CANDIDATES
        || request.budget_units == 0
        || request.max_actions == 0
        || usize::from(request.max_actions) > MAX_CANDIDATES
        || request.beam_width == 0
        || usize::from(request.beam_width) > MAX_BEAM_WIDTH
        || request.max_alternatives == 0
        || usize::from(request.max_alternatives) > MAX_ALTERNATIVES
        || weights_sum == 0
        || weights_sum > 10_000
    {
        return Err(DecisionValueError::InvalidRequest(
            "objective, bounded candidates, positive search/resource bounds, and nonzero weight budget are required".into(),
        ));
    }
    let known = request
        .candidates
        .iter()
        .map(|candidate| candidate.action_id.clone())
        .collect::<BTreeSet<_>>();
    if known.len() != request.candidates.len() {
        return Err(DecisionValueError::InvalidCandidate(
            "candidate action IDs must be unique".into(),
        ));
    }
    for candidate in &request.candidates {
        if candidate.action_id.trim().is_empty()
            || candidate.claim_id.trim().is_empty()
            || candidate.cost_units == 0
            || candidate.diversity_group.trim().is_empty()
            || !canonical(&candidate.depends_on)
            || candidate.depends_on.iter().any(|dependency| {
                dependency.trim().is_empty()
                    || dependency == &candidate.action_id
                    || !known.contains(dependency)
                    || dependency >= &candidate.action_id
            })
            || candidate.information_gain_milli > 1_000
            || candidate.uncertainty_reduction_milli > 1_000
            || candidate.contradiction_resolution_milli > 1_000
            || candidate.reproducibility_milli > 1_000
            || candidate.failure_probability_milli > 1_000
        {
            return Err(DecisionValueError::InvalidCandidate(
                "candidates require unique ordered dependencies, positive cost, bounded scores, and a diversity group".into(),
            ));
        }
    }
    Ok(())
}

fn validate_output(result: &DecisionValueResult) -> Result<(), DecisionValueError> {
    if result.feature_id != FEATURE_ID
        || result.output_schema != OUTPUT_SCHEMA
        || result.objective.trim().is_empty()
        || !canonical(&result.candidate_order)
        || !canonical(&result.selected_order)
        || !canonical(&result.deferred_order)
        || !canonical(&result.blocked_order)
        || !canonical(&result.negative_evidence_order)
        || !canonical(&result.uncertainty_order)
        || result.candidate_scores.len() != result.candidate_order.len()
        || result
            .candidate_scores
            .windows(2)
            .any(|pair| pair[0].action_id >= pair[1].action_id)
        || result.candidate_scores.iter().any(|score| {
            score.action_id.trim().is_empty()
                || score.reason_order.is_empty()
                || !canonical(&score.reason_order)
        })
        || result.alternatives.len() > MAX_ALTERNATIVES
        || result.alternatives.iter().any(|portfolio| {
            portfolio.action_order.is_empty()
                || !canonical(&portfolio.action_order)
                || !canonical(&portfolio.modality_order)
                || !canonical(&portfolio.model_system_order)
                || !canonical(&portfolio.group_order)
        })
    {
        return Err(DecisionValueError::InvalidOutput(
            "identity, canonical ordering, candidate coverage, score reason, or portfolio invariants are invalid".into(),
        ));
    }
    let expected = ContentHash::of_value(&digest_input(result))
        .map_err(|error| DecisionValueError::Digest(error.to_string()))?;
    if expected != result.digest {
        return Err(DecisionValueError::InvalidOutput(
            "decision value digest is not content-addressed".into(),
        ));
    }
    Ok(())
}

impl DecisionValueResult {
    pub fn validate(&self) -> Result<(), DecisionValueError> {
        validate_output(self)
    }
}

#[derive(Clone, Default)]
struct BeamState {
    action_order: Vec<String>,
    cost_units: u32,
    utility_milli: i32,
    modalities: BTreeSet<GliomaModality>,
    model_systems: BTreeSet<GliomaModelSystem>,
    groups: BTreeSet<String>,
}

fn state_cmp(left: &BeamState, right: &BeamState) -> std::cmp::Ordering {
    right
        .utility_milli
        .cmp(&left.utility_milli)
        .then_with(|| left.cost_units.cmp(&right.cost_units))
        .then_with(|| left.action_order.cmp(&right.action_order))
}

fn portfolio_from_state(state: &BeamState, index: usize) -> DecisionValuePortfolio {
    DecisionValuePortfolio {
        portfolio_id: format!("portfolio-{index:03}"),
        action_order: state.action_order.clone(),
        cost_units: state.cost_units,
        utility_milli: state.utility_milli,
        diversity_milli: (state.modalities.len() * 300
            + state.model_systems.len() * 300
            + state.groups.len() * 400)
            .min(1_000) as u16,
        modality_order: state.modalities.iter().copied().collect(),
        model_system_order: state.model_systems.iter().copied().collect(),
        group_order: state.groups.iter().cloned().collect(),
    }
}

/// Search a bounded beam of dependency-closed research action portfolios by expected value of
/// information and diversity, preserving alternatives and every blocked candidate reason.
pub fn optimize_glioma_decision_value(
    request: &DecisionValueRequest,
) -> Result<DecisionValueResult, DecisionValueError> {
    validate_request(request)?;
    let mut candidates = request.candidates.clone();
    candidates.sort_by_key(|candidate| candidate.action_id.clone());
    let mut beam = vec![BeamState::default()];
    let mut blocked = BTreeSet::new();
    let mut negative = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    let mut intrinsic = BTreeMap::new();
    for candidate in &candidates {
        let utility = weighted_utility(candidate, &request.weights);
        intrinsic.insert(candidate.action_id.clone(), utility);
        if !candidate.available {
            blocked.insert(candidate.action_id.clone());
            negative.insert(format!("unavailable:{}", candidate.action_id));
        }
        if utility < request.min_candidate_utility_milli {
            uncertainty.insert(format!("low-utility-candidate:{}", candidate.action_id));
        }
    }
    for candidate in &candidates {
        if !candidate.available
            || intrinsic[candidate.action_id.as_str()] < request.min_candidate_utility_milli
        {
            continue;
        }
        let mut next = beam.clone();
        for state in &beam {
            if state.action_order.len() >= usize::from(request.max_actions)
                || state.cost_units.saturating_add(candidate.cost_units) > request.budget_units
                || (request.require_dependency_closure
                    && candidate
                        .depends_on
                        .iter()
                        .any(|dependency| !state.action_order.binary_search(dependency).is_ok()))
            {
                if request.require_dependency_closure
                    && candidate
                        .depends_on
                        .iter()
                        .any(|dependency| !state.action_order.binary_search(dependency).is_ok())
                {
                    blocked.insert(candidate.action_id.clone());
                    negative.insert(format!(
                        "dependency-or-budget-bound:{}",
                        candidate.action_id
                    ));
                }
                continue;
            }
            let mut expanded = state.clone();
            expanded.action_order.push(candidate.action_id.clone());
            expanded.action_order.sort();
            expanded.cost_units = expanded.cost_units.saturating_add(candidate.cost_units);
            expanded.utility_milli = expanded
                .utility_milli
                .saturating_add(intrinsic[candidate.action_id.as_str()]);
            if expanded.modalities.insert(candidate.modality) {
                expanded.utility_milli = expanded
                    .utility_milli
                    .saturating_add(i32::from(request.weights.diversity) * 3);
            }
            if expanded.model_systems.insert(candidate.model_system) {
                expanded.utility_milli = expanded
                    .utility_milli
                    .saturating_add(i32::from(request.weights.diversity) * 3);
            }
            if expanded.groups.insert(candidate.diversity_group.clone()) {
                expanded.utility_milli = expanded
                    .utility_milli
                    .saturating_add(i32::from(request.weights.diversity) * 4);
            }
            next.push(expanded);
        }
        next.sort_by(state_cmp);
        next.dedup_by(|left, right| left.action_order == right.action_order);
        next.truncate(usize::from(request.beam_width));
        beam = next;
    }
    beam.retain(|state| !state.action_order.is_empty());
    beam.sort_by(state_cmp);
    let selected_state = beam.first().cloned();
    let selected_ids = selected_state
        .as_ref()
        .map(|state| state.action_order.iter().cloned().collect::<BTreeSet<_>>())
        .unwrap_or_default();
    let selected_portfolio = selected_state
        .as_ref()
        .map(|state| portfolio_from_state(state, 0));
    let alternatives = beam
        .iter()
        .skip(1)
        .take(usize::from(request.max_alternatives).saturating_sub(1))
        .enumerate()
        .map(|(index, state)| portfolio_from_state(state, index + 1))
        .collect::<Vec<_>>();
    let mut selected = BTreeSet::new();
    let mut deferred = BTreeSet::new();
    let mut scores = Vec::new();
    for candidate in &candidates {
        let action_id = &candidate.action_id;
        let intrinsic_utility = intrinsic[action_id.as_str()];
        let disposition = if selected_ids.contains(action_id) {
            selected.insert(action_id.clone());
            DecisionValueDisposition::Selected
        } else if blocked.contains(action_id) {
            DecisionValueDisposition::Blocked
        } else {
            deferred.insert(action_id.clone());
            DecisionValueDisposition::Deferred
        };
        let mut reasons = BTreeSet::new();
        match disposition {
            DecisionValueDisposition::Selected => {
                reasons.insert("selected-by-bounded-voi-beam".into());
            }
            DecisionValueDisposition::Deferred => {
                reasons.insert("dominated-or-budget-deferred".into());
            }
            DecisionValueDisposition::Blocked => {
                reasons.insert("dependency-availability-or-budget-blocked".into());
            }
            DecisionValueDisposition::Unresolved => {
                reasons.insert("unresolved-portfolio-state".into());
            }
        }
        scores.push(DecisionValueCandidateScore {
            action_id: action_id.clone(),
            intrinsic_utility_milli: intrinsic_utility,
            marginal_utility_milli: intrinsic_utility,
            disposition,
            reason_order: reasons.into_iter().collect(),
        });
    }
    let disposition = if selected_portfolio.is_some() {
        if blocked.is_empty() && deferred.is_empty() {
            DecisionValueCampaignDisposition::Ready
        } else if !blocked.is_empty() {
            DecisionValueCampaignDisposition::BudgetLimited
        } else {
            DecisionValueCampaignDisposition::Partial
        }
    } else if !blocked.is_empty() {
        DecisionValueCampaignDisposition::BudgetLimited
    } else {
        DecisionValueCampaignDisposition::Unresolved
    };
    let next_action = match disposition {
        DecisionValueCampaignDisposition::Ready => {
            "submit the selected value-of-information portfolio to decision admission"
        }
        DecisionValueCampaignDisposition::Partial => {
            "review deferred alternatives and submit only the selected bounded portfolio"
        }
        DecisionValueCampaignDisposition::BudgetLimited => {
            "resolve blocked dependencies or expand the declared research budget before selecting more actions"
        }
        DecisionValueCampaignDisposition::Unresolved => {
            "add available typed candidates or lower no gate without researcher review"
        }
    }
    .into();
    let mut result = DecisionValueResult {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        objective: request.objective.clone(),
        candidate_order: candidates
            .iter()
            .map(|candidate| candidate.action_id.clone())
            .collect(),
        selected_order: selected.into_iter().collect(),
        deferred_order: deferred.into_iter().collect(),
        blocked_order: blocked.into_iter().collect(),
        selected_portfolio,
        alternatives,
        candidate_scores: scores,
        negative_evidence_order: negative.into_iter().collect(),
        uncertainty_order: uncertainty.into_iter().collect(),
        disposition,
        next_action,
        digest: ContentHash::of_bytes(b"unsealed-glioma-decision-value"),
    };
    result.digest = ContentHash::of_value(&digest_input(&result))
        .map_err(|error| DecisionValueError::Digest(error.to_string()))?;
    validate_output(&result)?;
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn candidate(id: &str, modality: GliomaModality, cost: u32) -> DecisionValueCandidate {
        DecisionValueCandidate {
            action_id: id.into(),
            claim_id: "claim-invasion".into(),
            modality,
            model_system: GliomaModelSystem::Organoid,
            depends_on: Vec::new(),
            cost_units: cost,
            information_gain_milli: 800,
            uncertainty_reduction_milli: 700,
            contradiction_resolution_milli: 600,
            reproducibility_milli: 900,
            failure_probability_milli: 50,
            diversity_group: id.into(),
            available: true,
        }
    }

    fn request(candidates: Vec<DecisionValueCandidate>) -> DecisionValueRequest {
        DecisionValueRequest {
            objective: "choose the next glioma research portfolio by expected information value"
                .into(),
            candidates,
            budget_units: 6,
            max_actions: 2,
            beam_width: 8,
            max_alternatives: 3,
            require_dependency_closure: true,
            min_candidate_utility_milli: 100,
            weights: DecisionValueWeights {
                information_gain: 30,
                uncertainty_reduction: 25,
                contradiction_resolution: 20,
                reproducibility: 15,
                diversity: 10,
                failure_penalty: 10,
            },
        }
    }

    #[test]
    fn selects_bounded_portfolio_and_preserves_alternative() {
        let result = optimize_glioma_decision_value(&request(vec![
            candidate("a-genomics", GliomaModality::Genomics, 3),
            candidate("b-imaging", GliomaModality::Imaging, 3),
            candidate("c-expensive", GliomaModality::Proteomics, 8),
        ]))
        .expect("optimization");
        assert_eq!(result.selected_order.len(), 2);
        assert!(result.selected_portfolio.is_some());
        assert!(!result.alternatives.is_empty());
        result.validate().expect("digest and invariants");
    }

    #[test]
    fn blocks_unavailable_candidate_without_silent_selection() {
        let mut unavailable = candidate("unavailable", GliomaModality::Genomics, 2);
        unavailable.available = false;
        let result =
            optimize_glioma_decision_value(&request(vec![unavailable])).expect("optimization");
        assert!(result.blocked_order.contains(&"unavailable".into()));
        assert!(result
            .negative_evidence_order
            .iter()
            .any(|entry| entry.contains("unavailable")));
    }

    #[test]
    fn dependency_closure_blocks_dependent_without_prerequisite() {
        let mut prerequisite = candidate("a-prerequisite", GliomaModality::Genomics, 2);
        prerequisite.available = false;
        let mut dependent = candidate("b-dependent", GliomaModality::Imaging, 2);
        dependent.depends_on = vec!["a-prerequisite".into()];
        let result = optimize_glioma_decision_value(&request(vec![prerequisite, dependent]))
            .expect("optimization");
        assert!(result.blocked_order.contains(&"b-dependent".into()));
        assert!(result
            .negative_evidence_order
            .iter()
            .any(|entry| entry.contains("dependency")));
    }
}
