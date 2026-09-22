//! Sequential stopping and continuation policy for autonomous glioma research loops.
//!
//! The governor evaluates completed local rounds and decides whether the next round is worth
//! authorizing. Its net-progress score combines information gain and uncertainty reduction with
//! explicit penalties for failures, contradictions, and negative outcomes. It never hides a null
//! result: a round can be productive and still carry negative evidence, while unresolved review
//! states stop continuation when the declared policy requires it.

use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P04-F16";
pub const OUTPUT_SCHEMA: &str = "GliomaDecisionLoopGovernor1@1";
pub const MAX_ROUNDS: usize = 128;
pub const MAX_ACTIONS_PER_ROUND: usize = 64;
pub const MAX_ABS_METRIC_MILLI: i32 = 100_000;
pub const MAX_CUMULATIVE_PROGRESS_MILLI: i32 = MAX_ABS_METRIC_MILLI * MAX_ROUNDS as i32;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DecisionLoopRound {
    pub round_index: u16,
    pub action_order: Vec<String>,
    pub cost_units: u32,
    pub observed_information_gain_milli: i32,
    pub uncertainty_reduction_milli: i32,
    pub failure_count: u16,
    pub negative_count: u16,
    pub contradiction_count: u16,
    pub completed: bool,
    pub evidence_complete: bool,
    pub human_review_available: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DecisionLoopGovernorRequest {
    pub objective: String,
    pub rounds: Vec<DecisionLoopRound>,
    pub budget_units: u32,
    pub max_rounds: u16,
    pub min_progress_milli: i32,
    pub min_gain_milli: i32,
    pub max_failures: u16,
    pub require_negative_visibility: bool,
    pub allow_continue_on_partial: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DecisionLoopRoundDisposition {
    Productive,
    NoProgress,
    Negative,
    ReviewRequired,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DecisionLoopStopReason {
    Continue,
    Qualified,
    BudgetExhausted,
    NoProgress,
    FailureLimit,
    ReviewRequired,
    MaxRounds,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DecisionLoopGovernorDisposition {
    Continue,
    Qualified,
    PausedForReview,
    BudgetLimited,
    Unresolved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DecisionLoopRoundAssessment {
    pub round_index: u16,
    pub action_order: Vec<String>,
    pub cost_units: u32,
    pub net_progress_milli: i32,
    pub cumulative_progress_milli: i32,
    pub cumulative_cost_units: u32,
    pub disposition: DecisionLoopRoundDisposition,
    pub reason_order: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DecisionLoopGovernorResult {
    pub feature_id: String,
    pub output_schema: String,
    pub objective: String,
    pub assessments: Vec<DecisionLoopRoundAssessment>,
    pub total_progress_milli: i32,
    pub total_cost_units: u32,
    pub total_failures: u32,
    pub total_negative_results: u32,
    pub total_contradictions: u32,
    pub remaining_budget_units: u32,
    pub stop_reason: DecisionLoopStopReason,
    pub next_round: Option<u16>,
    pub negative_evidence_order: Vec<String>,
    pub uncertainty_order: Vec<String>,
    pub disposition: DecisionLoopGovernorDisposition,
    pub next_action: String,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum DecisionLoopGovernorError {
    #[error("decision loop governor request is invalid: {0}")]
    InvalidRequest(String),
    #[error("decision loop governor output is invalid: {0}")]
    InvalidOutput(String),
    #[error("decision loop governor digest failed: {0}")]
    Digest(String),
}

fn canonical<T: Ord>(values: &[T]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn bounded_metric(value: i32) -> bool {
    value.unsigned_abs() <= MAX_ABS_METRIC_MILLI as u32
}

fn digest_input(result: &DecisionLoopGovernorResult) -> serde_json::Value {
    serde_json::json!({
        "feature_id": result.feature_id,
        "output_schema": result.output_schema,
        "objective": result.objective,
        "assessments": result.assessments,
        "total_progress_milli": result.total_progress_milli,
        "total_cost_units": result.total_cost_units,
        "total_failures": result.total_failures,
        "total_negative_results": result.total_negative_results,
        "total_contradictions": result.total_contradictions,
        "remaining_budget_units": result.remaining_budget_units,
        "stop_reason": result.stop_reason,
        "next_round": result.next_round,
        "negative_evidence_order": result.negative_evidence_order,
        "uncertainty_order": result.uncertainty_order,
        "disposition": result.disposition,
        "next_action": result.next_action,
    })
}

fn validate_request(
    request: &DecisionLoopGovernorRequest,
) -> Result<(), DecisionLoopGovernorError> {
    if request.objective.trim().is_empty()
        || request.budget_units == 0
        || request.max_rounds == 0
        || usize::from(request.max_rounds) > MAX_ROUNDS
        || request.max_failures == 0
        || !bounded_metric(request.min_progress_milli)
        || !bounded_metric(request.min_gain_milli)
        || request.rounds.len() > MAX_ROUNDS
    {
        return Err(DecisionLoopGovernorError::InvalidRequest(
            "objective, positive budget/round/failure bounds, and bounded progress thresholds are required".into(),
        ));
    }
    for (position, round) in request.rounds.iter().enumerate() {
        if round.round_index != (position as u16).saturating_add(1)
            || round.action_order.is_empty()
            || round.action_order.len() > MAX_ACTIONS_PER_ROUND
            || !canonical(&round.action_order)
            || round.cost_units == 0
            || !bounded_metric(round.observed_information_gain_milli)
            || !bounded_metric(round.uncertainty_reduction_milli)
        {
            return Err(DecisionLoopGovernorError::InvalidRequest(
                "rounds require contiguous indexes, canonical action ids, positive cost, and bounded metrics".into(),
            ));
        }
    }
    Ok(())
}

fn validate_output(result: &DecisionLoopGovernorResult) -> Result<(), DecisionLoopGovernorError> {
    if result.feature_id != FEATURE_ID
        || result.output_schema != OUTPUT_SCHEMA
        || result.objective.trim().is_empty()
        || result.assessments.len() > MAX_ROUNDS
        || result
            .assessments
            .windows(2)
            .any(|pair| pair[0].round_index >= pair[1].round_index)
        || result.assessments.iter().any(|assessment| {
            assessment.round_index == 0
                || assessment.action_order.is_empty()
                || !canonical(&assessment.action_order)
                || assessment.cost_units == 0
                || !bounded_metric(assessment.net_progress_milli)
                || assessment.cumulative_progress_milli.unsigned_abs()
                    > MAX_CUMULATIVE_PROGRESS_MILLI as u32
                || assessment.reason_order.is_empty()
                || !canonical(&assessment.reason_order)
        })
        || !canonical(&result.negative_evidence_order)
        || !canonical(&result.uncertainty_order)
        || result.next_round.is_some_and(|round| round == 0)
        || result.next_action.trim().is_empty()
    {
        return Err(DecisionLoopGovernorError::InvalidOutput(
            "identity, assessment ordering, metrics, evidence, next-round, or action invariants are invalid".into(),
        ));
    }
    let expected = ContentHash::of_value(&digest_input(result))
        .map_err(|error| DecisionLoopGovernorError::Digest(error.to_string()))?;
    if expected != result.digest {
        return Err(DecisionLoopGovernorError::InvalidOutput(
            "decision loop digest is not content-addressed".into(),
        ));
    }
    Ok(())
}

impl DecisionLoopGovernorResult {
    pub fn validate(&self) -> Result<(), DecisionLoopGovernorError> {
        validate_output(self)
    }
}

/// Evaluate sequential local research rounds and produce a bounded continuation or stopping
/// decision for the autonomous engine.
pub fn govern_glioma_decision_loop(
    request: &DecisionLoopGovernorRequest,
) -> Result<DecisionLoopGovernorResult, DecisionLoopGovernorError> {
    validate_request(request)?;
    let mut assessments = Vec::with_capacity(request.rounds.len());
    let mut negative_evidence = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    let mut cumulative_progress = 0_i32;
    let mut total_cost = 0_u32;
    let mut total_failures = 0_u32;
    let mut total_negative = 0_u32;
    let mut total_contradictions = 0_u32;
    let mut no_progress_streak = 0_u16;
    let mut latest_evidence_complete = false;
    let mut stop_reason = DecisionLoopStopReason::Continue;

    for round in &request.rounds {
        total_cost = total_cost.saturating_add(round.cost_units);
        total_failures = total_failures.saturating_add(u32::from(round.failure_count));
        total_negative = total_negative.saturating_add(u32::from(round.negative_count));
        total_contradictions =
            total_contradictions.saturating_add(u32::from(round.contradiction_count));
        let failure_penalty = i32::from(round.failure_count).saturating_mul(500);
        let negative_penalty = i32::from(round.negative_count).saturating_mul(250);
        let contradiction_penalty = i32::from(round.contradiction_count).saturating_mul(200);
        let net_progress = round
            .observed_information_gain_milli
            .saturating_add(round.uncertainty_reduction_milli)
            .saturating_sub(failure_penalty)
            .saturating_sub(negative_penalty)
            .saturating_sub(contradiction_penalty);
        cumulative_progress = cumulative_progress.saturating_add(net_progress);
        let productive = round.completed
            && net_progress >= request.min_progress_milli
            && round.observed_information_gain_milli >= request.min_gain_milli;
        let review_required = round.contradiction_count > 0
            || (round.negative_count > 0 && request.require_negative_visibility)
            || (!round.completed && !request.allow_continue_on_partial);
        let disposition = if review_required {
            no_progress_streak = 0;
            DecisionLoopRoundDisposition::ReviewRequired
        } else if productive {
            no_progress_streak = 0;
            DecisionLoopRoundDisposition::Productive
        } else if round.failure_count > 0 || round.negative_count > 0 {
            no_progress_streak = no_progress_streak.saturating_add(1);
            DecisionLoopRoundDisposition::Negative
        } else {
            no_progress_streak = no_progress_streak.saturating_add(1);
            DecisionLoopRoundDisposition::NoProgress
        };
        if round.failure_count > 0 {
            negative_evidence.insert(format!(
                "round-{}:failures-{}",
                round.round_index, round.failure_count
            ));
        }
        if round.negative_count > 0 {
            negative_evidence.insert(format!(
                "round-{}:negative-results-{}",
                round.round_index, round.negative_count
            ));
        }
        if round.contradiction_count > 0 {
            negative_evidence.insert(format!(
                "round-{}:contradictions-{}",
                round.round_index, round.contradiction_count
            ));
        }
        if matches!(
            disposition,
            DecisionLoopRoundDisposition::NoProgress | DecisionLoopRoundDisposition::ReviewRequired
        ) {
            uncertainty.insert(format!("round-{}:{:?}", round.round_index, disposition));
        }
        let mut reasons = BTreeSet::new();
        reasons.insert(
            match disposition {
                DecisionLoopRoundDisposition::Productive => "progress-above-threshold",
                DecisionLoopRoundDisposition::NoProgress => "progress-below-threshold",
                DecisionLoopRoundDisposition::Negative => "negative-or-failed-outcome",
                DecisionLoopRoundDisposition::ReviewRequired => "review-gate-required",
            }
            .to_string(),
        );
        if round.evidence_complete {
            reasons.insert("evidence-complete-flag-observed".to_string());
            latest_evidence_complete = true;
        }
        if round.human_review_available {
            reasons.insert("human-review-available".to_string());
        }
        assessments.push(DecisionLoopRoundAssessment {
            round_index: round.round_index,
            action_order: round.action_order.clone(),
            cost_units: round.cost_units,
            net_progress_milli: net_progress,
            cumulative_progress_milli: cumulative_progress,
            cumulative_cost_units: total_cost,
            disposition,
            reason_order: reasons.into_iter().collect(),
        });
    }
    let remaining_budget = request.budget_units.saturating_sub(total_cost);
    if latest_evidence_complete
        && cumulative_progress >= request.min_progress_milli
        && total_contradictions == 0
    {
        stop_reason = DecisionLoopStopReason::Qualified;
    } else if total_cost >= request.budget_units {
        stop_reason = DecisionLoopStopReason::BudgetExhausted;
    } else if total_failures >= u32::from(request.max_failures) {
        stop_reason = DecisionLoopStopReason::FailureLimit;
    } else if assessments.iter().rev().any(|assessment| {
        matches!(
            assessment.disposition,
            DecisionLoopRoundDisposition::ReviewRequired
        )
    }) && !request.allow_continue_on_partial
    {
        stop_reason = DecisionLoopStopReason::ReviewRequired;
    } else if no_progress_streak >= 2 {
        stop_reason = DecisionLoopStopReason::NoProgress;
    } else if assessments.len() >= usize::from(request.max_rounds) {
        stop_reason = DecisionLoopStopReason::MaxRounds;
    }
    let disposition = match stop_reason {
        DecisionLoopStopReason::Continue => DecisionLoopGovernorDisposition::Continue,
        DecisionLoopStopReason::Qualified => DecisionLoopGovernorDisposition::Qualified,
        DecisionLoopStopReason::BudgetExhausted => DecisionLoopGovernorDisposition::BudgetLimited,
        DecisionLoopStopReason::ReviewRequired | DecisionLoopStopReason::FailureLimit => {
            DecisionLoopGovernorDisposition::PausedForReview
        }
        DecisionLoopStopReason::NoProgress | DecisionLoopStopReason::MaxRounds => {
            DecisionLoopGovernorDisposition::Unresolved
        }
    };
    let next_round =
        (stop_reason == DecisionLoopStopReason::Continue).then(|| assessments.len() as u16 + 1);
    let next_action = match stop_reason {
        DecisionLoopStopReason::Continue => {
            "compile and admit the next bounded portfolio, then re-evaluate observed progress"
        }
        DecisionLoopStopReason::Qualified => {
            "freeze the qualified local result and send it to independent validation and release review"
        }
        DecisionLoopStopReason::BudgetExhausted => {
            "stop dispatch and resume only with an explicit additional research budget"
        }
        DecisionLoopStopReason::NoProgress => {
            "stop repeated dispatch and revise the research intent or supply a new discriminating action"
        }
        DecisionLoopStopReason::FailureLimit => {
            "pause autonomous continuation and review repeated execution failures before retrying"
        }
        DecisionLoopStopReason::ReviewRequired => {
            "pause for researcher review of contradictions, negative results, or incomplete closure"
        }
        DecisionLoopStopReason::MaxRounds => {
            "resume only with a new bounded continuation request and explicit round budget"
        }
    }
    .to_string();
    let mut result = DecisionLoopGovernorResult {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        objective: request.objective.clone(),
        assessments,
        total_progress_milli: cumulative_progress,
        total_cost_units: total_cost,
        total_failures,
        total_negative_results: total_negative,
        total_contradictions,
        remaining_budget_units: remaining_budget,
        stop_reason,
        next_round,
        negative_evidence_order: negative_evidence.into_iter().collect(),
        uncertainty_order: uncertainty.into_iter().collect(),
        disposition,
        next_action,
        digest: ContentHash::of_bytes(b"unsealed-glioma-decision-loop-governor"),
    };
    result.digest = ContentHash::of_value(&digest_input(&result))
        .map_err(|error| DecisionLoopGovernorError::Digest(error.to_string()))?;
    validate_output(&result)?;
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn round(
        index: u16,
        gain: i32,
        uncertainty: i32,
        failures: u16,
        negative: u16,
        contradictions: u16,
        complete: bool,
    ) -> DecisionLoopRound {
        DecisionLoopRound {
            round_index: index,
            action_order: vec![format!("action-{index:02}")],
            cost_units: 3,
            observed_information_gain_milli: gain,
            uncertainty_reduction_milli: uncertainty,
            failure_count: failures,
            negative_count: negative,
            contradiction_count: contradictions,
            completed: complete,
            evidence_complete: complete,
            human_review_available: false,
        }
    }

    fn request(rounds: Vec<DecisionLoopRound>) -> DecisionLoopGovernorRequest {
        DecisionLoopGovernorRequest {
            objective: "govern a bounded glioma research loop".into(),
            rounds,
            budget_units: 12,
            max_rounds: 4,
            min_progress_milli: 500,
            min_gain_milli: 250,
            max_failures: 3,
            require_negative_visibility: true,
            allow_continue_on_partial: false,
        }
    }

    #[test]
    fn productive_complete_round_qualifies_without_contradiction() {
        let result = govern_glioma_decision_loop(&request(vec![round(1, 800, 400, 0, 0, 0, true)]))
            .expect("governor");
        assert_eq!(result.stop_reason, DecisionLoopStopReason::Qualified);
        assert_eq!(
            result.disposition,
            DecisionLoopGovernorDisposition::Qualified
        );
        result.validate().expect("digest and invariants");
    }

    #[test]
    fn repeated_no_progress_stops_continuation() {
        let result = govern_glioma_decision_loop(&request(vec![
            round(1, 0, 0, 0, 0, 0, true),
            round(2, 0, 0, 0, 0, 0, true),
        ]))
        .expect("governor");
        assert_eq!(result.stop_reason, DecisionLoopStopReason::NoProgress);
        assert!(result
            .uncertainty_order
            .iter()
            .any(|item| item.contains("round-2")));
    }

    #[test]
    fn contradictions_and_negative_results_pause_for_review() {
        let result = govern_glioma_decision_loop(&request(vec![round(1, 800, 400, 0, 1, 1, true)]))
            .expect("governor");
        assert_eq!(result.stop_reason, DecisionLoopStopReason::ReviewRequired);
        assert!(result
            .negative_evidence_order
            .iter()
            .any(|item| item.contains("contradictions")));
        assert!(result
            .negative_evidence_order
            .iter()
            .any(|item| item.contains("negative-results")));
    }
}
