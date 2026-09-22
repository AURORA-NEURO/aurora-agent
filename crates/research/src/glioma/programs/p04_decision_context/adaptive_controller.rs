//! Adaptive exploitation/exploration control for the glioma decision portfolio.
//!
//! This module is the executable bridge between outcome calibration and portfolio selection. It
//! does not blindly maximize the learned score: prior-only candidates receive a bounded
//! exploration bonus, conflicted forecasts receive an explicit penalty while remaining visible,
//! and all choices still pass through dependency, budget, availability, and bounded beam gates.
//! The result is a planning portfolio for the next local research round, never a biological or
//! clinical conclusion.

use super::value_calibration::{
    calibrate_glioma_decision_value, DecisionValueCalibrationCampaignDisposition,
    DecisionValueCalibrationDisposition, DecisionValueCalibrationError,
    DecisionValueCalibrationRequest, DecisionValueCalibrationResult,
};
use super::value_optimizer::{weighted_utility, DecisionValueDisposition, DecisionValuePortfolio};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P04-F15";
pub const OUTPUT_SCHEMA: &str = "GliomaAdaptiveDecisionController1@1";
pub const MAX_CANDIDATES: usize = 256;
pub const MAX_BEAM_WIDTH: usize = 512;
pub const MAX_ALTERNATIVES: usize = 16;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdaptiveDecisionControllerRequest {
    pub calibration: DecisionValueCalibrationRequest,
    pub budget_units: u32,
    pub max_actions: u16,
    pub beam_width: u16,
    pub max_alternatives: u16,
    pub require_dependency_closure: bool,
    pub exploration_weight_milli: u16,
    pub min_controller_utility_milli: i32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdaptiveDecisionCandidateScore {
    pub action_id: String,
    pub prior_utility_milli: i32,
    pub calibrated_utility_milli: i32,
    pub exploration_bonus_milli: i32,
    pub conflict_penalty_milli: i32,
    pub controller_utility_milli: i32,
    pub confidence_milli: u16,
    pub calibration_disposition: DecisionValueCalibrationDisposition,
    pub disposition: DecisionValueDisposition,
    pub reason_order: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AdaptiveDecisionCampaignDisposition {
    Ready,
    Partial,
    BudgetLimited,
    CalibrationReview,
    Unresolved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdaptiveDecisionControllerResult {
    pub feature_id: String,
    pub output_schema: String,
    pub objective: String,
    pub calibration: DecisionValueCalibrationResult,
    pub candidate_order: Vec<String>,
    pub selected_order: Vec<String>,
    pub deferred_order: Vec<String>,
    pub blocked_order: Vec<String>,
    pub selected_portfolio: Option<DecisionValuePortfolio>,
    pub alternatives: Vec<DecisionValuePortfolio>,
    pub candidate_scores: Vec<AdaptiveDecisionCandidateScore>,
    pub negative_evidence_order: Vec<String>,
    pub uncertainty_order: Vec<String>,
    pub disposition: AdaptiveDecisionCampaignDisposition,
    pub next_action: String,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum AdaptiveDecisionControllerError {
    #[error("adaptive decision controller request is invalid: {0}")]
    InvalidRequest(String),
    #[error("adaptive decision controller calibration failed: {0}")]
    Calibration(#[from] DecisionValueCalibrationError),
    #[error("adaptive decision controller output is invalid: {0}")]
    InvalidOutput(String),
    #[error("adaptive decision controller digest failed: {0}")]
    Digest(String),
}

fn canonical<T: Ord>(values: &[T]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn validate_request(
    request: &AdaptiveDecisionControllerRequest,
) -> Result<(), AdaptiveDecisionControllerError> {
    if request.budget_units == 0
        || request.max_actions == 0
        || usize::from(request.max_actions) > MAX_CANDIDATES
        || request.beam_width == 0
        || usize::from(request.beam_width) > MAX_BEAM_WIDTH
        || request.max_alternatives == 0
        || usize::from(request.max_alternatives) > MAX_ALTERNATIVES
        || request.exploration_weight_milli > 10_000
        || request.calibration.candidates.len() > MAX_CANDIDATES
    {
        return Err(AdaptiveDecisionControllerError::InvalidRequest(
            "positive bounded budget/action/beam/alternative limits and bounded exploration are required".into(),
        ));
    }
    Ok(())
}

fn digest_input(result: &AdaptiveDecisionControllerResult) -> serde_json::Value {
    serde_json::json!({
        "feature_id": result.feature_id,
        "output_schema": result.output_schema,
        "objective": result.objective,
        "calibration": result.calibration,
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

fn validate_portfolio(portfolio: &DecisionValuePortfolio) -> bool {
    !portfolio.action_order.is_empty()
        && canonical(&portfolio.action_order)
        && canonical(&portfolio.modality_order)
        && canonical(&portfolio.model_system_order)
        && canonical(&portfolio.group_order)
}

fn validate_output(
    result: &AdaptiveDecisionControllerResult,
) -> Result<(), AdaptiveDecisionControllerError> {
    result.calibration.validate()?;
    let mut portfolio_ids = BTreeSet::new();
    if result.feature_id != FEATURE_ID
        || result.output_schema != OUTPUT_SCHEMA
        || result.objective.trim().is_empty()
        || !canonical(&result.candidate_order)
        || result.candidate_order.is_empty()
        || !canonical(&result.selected_order)
        || !canonical(&result.deferred_order)
        || !canonical(&result.blocked_order)
        || !canonical(&result.negative_evidence_order)
        || !canonical(&result.uncertainty_order)
        || result
            .candidate_scores
            .windows(2)
            .any(|pair| pair[0].action_id >= pair[1].action_id)
        || result.candidate_scores.len() != result.candidate_order.len()
        || result.candidate_scores.iter().any(|score| {
            score.action_id.trim().is_empty()
                || score.reason_order.is_empty()
                || !canonical(&score.reason_order)
        })
        || result.candidate_scores.iter().any(|score| {
            !result
                .candidate_order
                .binary_search(&score.action_id)
                .is_ok()
        })
        || result
            .selected_portfolio
            .as_ref()
            .is_some_and(|portfolio| !validate_portfolio(portfolio))
        || result.alternatives.len() > MAX_ALTERNATIVES
        || result.alternatives.iter().any(|portfolio| {
            !validate_portfolio(portfolio) || !portfolio_ids.insert(portfolio.portfolio_id.clone())
        })
        || result
            .selected_portfolio
            .as_ref()
            .is_some_and(|portfolio| !portfolio_ids.insert(portfolio.portfolio_id.clone()))
        || result.next_action.trim().is_empty()
    {
        return Err(AdaptiveDecisionControllerError::InvalidOutput(
            "identity, calibration, ordering, score, candidate, portfolio, or next-action invariants are invalid".into(),
        ));
    }
    let expected = ContentHash::of_value(&digest_input(result))
        .map_err(|error| AdaptiveDecisionControllerError::Digest(error.to_string()))?;
    if expected != result.digest {
        return Err(AdaptiveDecisionControllerError::InvalidOutput(
            "adaptive controller digest is not content-addressed".into(),
        ));
    }
    Ok(())
}

impl AdaptiveDecisionControllerResult {
    pub fn validate(&self) -> Result<(), AdaptiveDecisionControllerError> {
        validate_output(self)
    }
}

#[derive(Clone, Default)]
struct BeamState {
    action_order: Vec<String>,
    cost_units: u32,
    utility_milli: i32,
    exploration_milli: i32,
    confidence_sum_milli: u32,
    modalities: BTreeSet<crate::glioma_engine::GliomaModality>,
    model_systems: BTreeSet<crate::glioma_engine::GliomaModelSystem>,
    groups: BTreeSet<String>,
}

fn state_cmp(left: &BeamState, right: &BeamState) -> std::cmp::Ordering {
    right
        .utility_milli
        .cmp(&left.utility_milli)
        .then_with(|| right.confidence_sum_milli.cmp(&left.confidence_sum_milli))
        .then_with(|| left.cost_units.cmp(&right.cost_units))
        .then_with(|| left.action_order.cmp(&right.action_order))
}

fn portfolio_from_state(state: &BeamState, index: usize) -> DecisionValuePortfolio {
    DecisionValuePortfolio {
        portfolio_id: format!("adaptive-portfolio-{index:03}"),
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

/// Select an adaptive next-action portfolio from calibrated local outcomes and bounded
/// exploration. Prior-only candidates are explored only within the declared exploration budget;
/// conflicts remain visible and cannot be silently treated as successful evidence.
pub fn execute_glioma_adaptive_decision_controller(
    request: &AdaptiveDecisionControllerRequest,
) -> Result<AdaptiveDecisionControllerResult, AdaptiveDecisionControllerError> {
    validate_request(request)?;
    let calibration = calibrate_glioma_decision_value(&request.calibration)?;
    let calibration_by_action = calibration
        .records
        .iter()
        .map(|record| (record.action_id.clone(), record))
        .collect::<BTreeMap<_, _>>();
    let mut candidates = request.calibration.candidates.clone();
    candidates.sort_by_key(|candidate| candidate.action_id.clone());
    let mut adaptive_utility = BTreeMap::new();
    let mut score_records = Vec::with_capacity(candidates.len());
    let mut blocked = BTreeSet::new();
    let mut uncertainty = calibration
        .uncertainty_order
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let mut negative = calibration
        .negative_evidence_order
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    for candidate in &candidates {
        let record = calibration_by_action
            .get(&candidate.action_id)
            .ok_or_else(|| {
                AdaptiveDecisionControllerError::InvalidOutput(format!(
                    "calibration omitted candidate {}",
                    candidate.action_id
                ))
            })?;
        let prior = weighted_utility(candidate, &request.calibration.weights);
        let confidence = i32::from(record.confidence_milli);
        let exploration = (i32::from(request.exploration_weight_milli)
            .saturating_mul(1_000 - confidence)
            / 1_000)
            .max(0);
        let calibrated = record.calibrated_utility_milli;
        let conflict_penalty = if matches!(
            record.disposition,
            DecisionValueCalibrationDisposition::Conflicted
        ) {
            i32::from(request.exploration_weight_milli)
        } else if matches!(
            record.disposition,
            DecisionValueCalibrationDisposition::Unreliable
        ) {
            i32::from(request.exploration_weight_milli) / 2
        } else {
            0
        };
        let base = if matches!(
            record.disposition,
            DecisionValueCalibrationDisposition::Calibrated
        ) && record.confidence_milli >= request.calibration.min_confidence_milli
        {
            calibrated
        } else {
            prior
        };
        let utility = base
            .saturating_add(exploration)
            .saturating_sub(conflict_penalty);
        adaptive_utility.insert(candidate.action_id.clone(), utility);
        if matches!(
            record.disposition,
            DecisionValueCalibrationDisposition::Conflicted
        ) {
            negative.insert(format!("controller-conflicted:{}", candidate.action_id));
            uncertainty.insert(format!("controller-review:{}", candidate.action_id));
        }
        if !candidate.available {
            blocked.insert(candidate.action_id.clone());
        }
        score_records.push((
            candidate,
            record,
            prior,
            exploration,
            conflict_penalty,
            utility,
        ));
    }
    for candidate in &candidates {
        if candidate.depends_on.iter().any(|dependency| {
            !candidates
                .iter()
                .any(|other| other.action_id == *dependency)
        }) {
            return Err(AdaptiveDecisionControllerError::InvalidRequest(format!(
                "candidate {} has an unknown dependency",
                candidate.action_id
            )));
        }
    }
    let mut beam = vec![BeamState::default()];
    for candidate in &candidates {
        let utility = adaptive_utility[&candidate.action_id];
        if !candidate.available || utility < request.min_controller_utility_milli {
            continue;
        }
        let mut next = beam.clone();
        for state in &beam {
            let missing_dependency = request.require_dependency_closure
                && candidate
                    .depends_on
                    .iter()
                    .any(|dependency| !state.action_order.binary_search(dependency).is_ok());
            if state.action_order.len() >= usize::from(request.max_actions)
                || state.cost_units.saturating_add(candidate.cost_units) > request.budget_units
                || missing_dependency
            {
                if missing_dependency {
                    blocked.insert(candidate.action_id.clone());
                    negative.insert(format!("dependency-blocked:{}", candidate.action_id));
                }
                continue;
            }
            let mut expanded = state.clone();
            expanded.action_order.push(candidate.action_id.clone());
            expanded.action_order.sort();
            expanded.cost_units = expanded.cost_units.saturating_add(candidate.cost_units);
            expanded.utility_milli = expanded.utility_milli.saturating_add(utility);
            let record = calibration_by_action[&candidate.action_id];
            expanded.confidence_sum_milli = expanded
                .confidence_sum_milli
                .saturating_add(u32::from(record.confidence_milli));
            expanded.exploration_milli = expanded
                .exploration_milli
                .saturating_add(i32::from(request.exploration_weight_milli));
            if expanded.modalities.insert(candidate.modality) {
                expanded.utility_milli = expanded
                    .utility_milli
                    .saturating_add(i32::from(request.calibration.weights.diversity) * 3);
            }
            if expanded.model_systems.insert(candidate.model_system) {
                expanded.utility_milli = expanded
                    .utility_milli
                    .saturating_add(i32::from(request.calibration.weights.diversity) * 3);
            }
            if expanded.groups.insert(candidate.diversity_group.clone()) {
                expanded.utility_milli = expanded
                    .utility_milli
                    .saturating_add(i32::from(request.calibration.weights.diversity) * 4);
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
    let mut scores = Vec::with_capacity(candidates.len());
    for (candidate, calibration_record, prior, exploration, conflict_penalty, utility) in
        score_records
    {
        let disposition = if selected_ids.contains(&candidate.action_id) {
            selected.insert(candidate.action_id.clone());
            DecisionValueDisposition::Selected
        } else if blocked.contains(&candidate.action_id) {
            DecisionValueDisposition::Blocked
        } else {
            deferred.insert(candidate.action_id.clone());
            DecisionValueDisposition::Deferred
        };
        let mut reasons = BTreeSet::new();
        match disposition {
            DecisionValueDisposition::Selected => {
                reasons.insert("selected-by-adaptive-controller".to_string());
            }
            DecisionValueDisposition::Deferred => {
                reasons.insert("deferred-by-bounded-adaptive-ranking".to_string());
            }
            DecisionValueDisposition::Blocked => {
                reasons.insert("blocked-by-availability-dependency-or-budget".to_string());
            }
            DecisionValueDisposition::Unresolved => {
                reasons.insert("unresolved-adaptive-state".to_string());
            }
        }
        if matches!(
            calibration_record.disposition,
            DecisionValueCalibrationDisposition::PriorOnly
        ) {
            reasons.insert("exploration-bonus-for-prior-only".to_string());
        }
        if conflict_penalty > 0 {
            reasons.insert("forecast-conflict-penalty-applied".to_string());
        }
        scores.push(AdaptiveDecisionCandidateScore {
            action_id: candidate.action_id.clone(),
            prior_utility_milli: prior,
            calibrated_utility_milli: calibration_record.calibrated_utility_milli,
            exploration_bonus_milli: exploration,
            conflict_penalty_milli: conflict_penalty,
            controller_utility_milli: utility,
            confidence_milli: calibration_record.confidence_milli,
            calibration_disposition: calibration_record.disposition,
            disposition,
            reason_order: reasons.into_iter().collect(),
        });
    }
    let disposition = if selected_portfolio.is_none() {
        if blocked.is_empty() {
            AdaptiveDecisionCampaignDisposition::Unresolved
        } else {
            AdaptiveDecisionCampaignDisposition::BudgetLimited
        }
    } else if matches!(
        calibration.disposition,
        DecisionValueCalibrationCampaignDisposition::Conflicted
    ) {
        AdaptiveDecisionCampaignDisposition::CalibrationReview
    } else if !blocked.is_empty() || !deferred.is_empty() {
        AdaptiveDecisionCampaignDisposition::Partial
    } else {
        AdaptiveDecisionCampaignDisposition::Ready
    };
    let next_action = match disposition {
        AdaptiveDecisionCampaignDisposition::Ready => {
            "submit the adaptive portfolio to decision admission"
        }
        AdaptiveDecisionCampaignDisposition::Partial => {
            "review deferred and blocked candidates before submitting the selected adaptive portfolio"
        }
        AdaptiveDecisionCampaignDisposition::BudgetLimited => {
            "resolve blocked dependencies or expand the bounded research budget"
        }
        AdaptiveDecisionCampaignDisposition::CalibrationReview => {
            "review forecast conflicts and negative outcomes before authorizing adaptive execution"
        }
        AdaptiveDecisionCampaignDisposition::Unresolved => {
            "add available typed candidates or relax no controller gate only with researcher review"
        }
    }
    .to_string();
    let mut result = AdaptiveDecisionControllerResult {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        objective: request.calibration.objective.clone(),
        calibration,
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
        digest: ContentHash::of_bytes(b"unsealed-glioma-adaptive-decision-controller"),
    };
    result.digest = ContentHash::of_value(&digest_input(&result))
        .map_err(|error| AdaptiveDecisionControllerError::Digest(error.to_string()))?;
    validate_output(&result)?;
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::super::value_optimizer::{DecisionValueCandidate, DecisionValueWeights};
    use super::*;
    use crate::glioma_engine::{GliomaModality, GliomaModelSystem};

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

    fn request(
        candidates: Vec<DecisionValueCandidate>,
        observations: Vec<super::super::value_calibration::DecisionValueObservation>,
    ) -> AdaptiveDecisionControllerRequest {
        AdaptiveDecisionControllerRequest {
            calibration: DecisionValueCalibrationRequest {
                objective: "adapt the next glioma research portfolio from local outcomes".into(),
                candidates,
                observations,
                weights: DecisionValueWeights {
                    information_gain: 30,
                    uncertainty_reduction: 25,
                    contradiction_resolution: 20,
                    reproducibility: 15,
                    diversity: 10,
                    failure_penalty: 10,
                },
                shrinkage_weight: 10,
                min_confidence_milli: 100,
                conflict_error_milli: 3_000,
                max_results: 8,
            },
            budget_units: 6,
            max_actions: 2,
            beam_width: 8,
            max_alternatives: 3,
            require_dependency_closure: true,
            exploration_weight_milli: 400,
            min_controller_utility_milli: 100,
        }
    }

    fn observation(
        action_id: &str,
        run_id: &str,
        failed: bool,
    ) -> super::super::value_calibration::DecisionValueObservation {
        super::super::value_calibration::DecisionValueObservation {
            action_id: action_id.into(),
            run_id: run_id.into(),
            predicted_utility_milli: 745,
            observed_information_gain_milli: 900,
            observed_uncertainty_reduction_milli: 800,
            observed_contradiction_resolution_milli: 700,
            observed_reproducibility_milli: 900,
            failed,
            sample_weight: 2,
        }
    }

    #[test]
    fn adaptive_controller_selects_portfolio_and_preserves_prior_only_exploration() {
        let result = execute_glioma_adaptive_decision_controller(&request(
            vec![
                candidate("a-observed", GliomaModality::Genomics, 3),
                candidate("b-prior-only", GliomaModality::Imaging, 3),
                candidate("c-over-budget", GliomaModality::Proteomics, 8),
            ],
            vec![observation("a-observed", "run-001", false)],
        ))
        .expect("adaptive controller");
        assert!(result.selected_portfolio.is_some());
        assert_eq!(result.selected_order.len(), 2);
        assert!(result
            .candidate_scores
            .iter()
            .any(|score| score.action_id == "b-prior-only"
                && score
                    .reason_order
                    .iter()
                    .any(|reason| reason.contains("exploration-bonus"))));
        result.validate().expect("digest and invariants");
    }

    #[test]
    fn adaptive_controller_surfaces_conflicted_outcome_for_review() {
        let mut failed = observation("a-conflicted", "run-002", true);
        failed.predicted_utility_milli = 9_000;
        failed.observed_information_gain_milli = 0;
        failed.observed_uncertainty_reduction_milli = 0;
        failed.observed_contradiction_resolution_milli = 0;
        failed.observed_reproducibility_milli = 0;
        let result = execute_glioma_adaptive_decision_controller(&request(
            vec![candidate("a-conflicted", GliomaModality::Imaging, 2)],
            vec![failed],
        ))
        .expect("adaptive controller");
        assert_eq!(
            result.disposition,
            AdaptiveDecisionCampaignDisposition::CalibrationReview
        );
        assert!(result
            .negative_evidence_order
            .iter()
            .any(|item| item.contains("failed:a-conflicted:run-002")));
        assert!(result
            .candidate_scores
            .iter()
            .any(|score| score.conflict_penalty_milli > 0));
    }

    #[test]
    fn adaptive_controller_blocks_missing_dependency_without_selection() {
        let mut prerequisite = candidate("a-prerequisite", GliomaModality::Genomics, 2);
        prerequisite.available = false;
        let mut dependent = candidate("b-dependent", GliomaModality::Imaging, 2);
        dependent.depends_on = vec!["a-prerequisite".into()];
        let result = execute_glioma_adaptive_decision_controller(&request(
            vec![prerequisite, dependent],
            Vec::new(),
        ))
        .expect("adaptive controller");
        assert!(result
            .blocked_order
            .iter()
            .any(|item| item == "b-dependent"));
        assert!(result
            .negative_evidence_order
            .iter()
            .any(|item| item.contains("dependency-blocked")));
    }
}
