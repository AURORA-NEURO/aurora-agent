//! Adaptive refresh scheduling for the autonomous glioma research director.
//!
//! A context is not static: new results, contradictions, and age can invalidate the action
//! frontier that was compiled from it. This feature turns those signals into a bounded local
//! refresh schedule. It never refreshes by itself and never treats a deferred context as current;
//! every omission carries a typed reason and critical omissions change the schedule disposition.

use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P04-F31";
pub const OUTPUT_SCHEMA: &str = "GliomaContextRefreshSchedule1@2";
pub const MAX_CANDIDATES: usize = 4_096;
pub const MAX_ACTIONS: usize = 1_024;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContextRefreshCandidate {
    pub context_id: String,
    pub context_digest: ContentHash,
    pub fairness_group: String,
    pub last_refresh_epoch: u64,
    pub latest_dependency_epoch: u64,
    pub staleness_horizon_epochs: u64,
    pub contradiction_milli: u16,
    pub new_result_count: u32,
    pub base_priority_milli: u16,
    pub cost_units: u32,
    pub critical: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContextRefreshScheduleRequest {
    pub objective: String,
    pub now_epoch: u64,
    pub budget_units: u64,
    pub max_actions: usize,
    pub contradiction_trigger_milli: u16,
    pub starvation_bound_epochs: u64,
    pub candidates: Vec<ContextRefreshCandidate>,
    pub replay_identity: ContentHash,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContextRefreshReason {
    Stale,
    Contradictory,
    NewResult,
    Starved,
    PriorityDeclared,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContextRefreshDeferralReason {
    Budget,
    ActionLimit,
    NotDue,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContextRefreshAction {
    pub action_id: String,
    pub context_id: String,
    pub context_digest: ContentHash,
    pub fairness_group: String,
    pub reason_order: Vec<ContextRefreshReason>,
    pub priority_milli: u16,
    pub cost_units: u32,
    pub critical: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContextRefreshDeferral {
    pub context_id: String,
    pub fairness_group: String,
    pub reason: ContextRefreshDeferralReason,
    pub priority_milli: u16,
    pub cost_units: u32,
    pub critical: bool,
    pub rationale: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContextRefreshScheduleDisposition {
    Ready,
    Partial,
    BudgetBlocked,
    Unresolved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContextRefreshSchedule {
    pub feature_id: String,
    pub output_schema: String,
    pub objective: String,
    pub now_epoch: u64,
    pub action_order: Vec<String>,
    pub deferred_order: Vec<String>,
    pub actions: Vec<ContextRefreshAction>,
    pub deferrals: Vec<ContextRefreshDeferral>,
    pub budget_units: u64,
    pub budget_used_units: u64,
    pub budget_remaining_units: u64,
    pub disposition: ContextRefreshScheduleDisposition,
    pub uncertainty: Vec<String>,
    pub next_action: String,
    pub replay_identity: ContentHash,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ContextRefreshScheduleError {
    #[error("context refresh request is invalid: {0}")]
    InvalidRequest(String),
    #[error("context refresh schedule is invalid: {0}")]
    InvalidOutput(String),
    #[error("context refresh schedule digest failed: {0}")]
    Digest(String),
}

fn canonical<T: Ord>(values: &[T]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn bounded_text(value: &str) -> bool {
    !value.trim().is_empty() && value.len() <= 4_096
}

fn digest_input(schedule: &ContextRefreshSchedule) -> serde_json::Value {
    serde_json::json!({
        "feature_id": schedule.feature_id,
        "output_schema": schedule.output_schema,
        "objective": schedule.objective,
        "now_epoch": schedule.now_epoch,
        "action_order": schedule.action_order,
        "deferred_order": schedule.deferred_order,
        "actions": schedule.actions,
        "deferrals": schedule.deferrals,
        "budget_units": schedule.budget_units,
        "budget_used_units": schedule.budget_used_units,
        "budget_remaining_units": schedule.budget_remaining_units,
        "disposition": schedule.disposition,
        "uncertainty": schedule.uncertainty,
        "next_action": schedule.next_action,
        "replay_identity": schedule.replay_identity,
    })
}

fn validate_request(
    request: &ContextRefreshScheduleRequest,
) -> Result<(), ContextRefreshScheduleError> {
    if !bounded_text(&request.objective)
        || request.now_epoch == 0
        || request.budget_units == 0
        || request.max_actions == 0
        || request.max_actions > MAX_ACTIONS
        || request.contradiction_trigger_milli == 0
        || request.contradiction_trigger_milli > 1_000
        || request.starvation_bound_epochs == 0
        || request.candidates.len() > MAX_CANDIDATES
        || request.replay_identity.as_str().len() != 64
    {
        return Err(ContextRefreshScheduleError::InvalidRequest(
            "objective, epoch, positive budget/action limits, contradiction threshold, starvation bound, and replay identity are required".into(),
        ));
    }
    let mut context_ids = BTreeSet::new();
    for candidate in &request.candidates {
        if !bounded_text(&candidate.context_id)
            || candidate.context_digest.as_str().len() != 64
            || !bounded_text(&candidate.fairness_group)
            || candidate.last_refresh_epoch > request.now_epoch
            || candidate.latest_dependency_epoch > request.now_epoch
            || candidate.staleness_horizon_epochs == 0
            || candidate.contradiction_milli > 1_000
            || candidate.base_priority_milli > 1_000
            || candidate.cost_units == 0
            || !context_ids.insert(candidate.context_id.clone())
        {
            return Err(ContextRefreshScheduleError::InvalidRequest(
                "context candidates require unique bounded identity, local digest, valid epochs, positive horizon/cost, and bounded scores".into(),
            ));
        }
    }
    Ok(())
}

fn candidate_reasons(
    candidate: &ContextRefreshCandidate,
    request: &ContextRefreshScheduleRequest,
) -> Vec<ContextRefreshReason> {
    let stale = request
        .now_epoch
        .saturating_sub(candidate.last_refresh_epoch)
        > candidate.staleness_horizon_epochs;
    let dependency_changed = candidate.latest_dependency_epoch > candidate.last_refresh_epoch;
    let contradictory = candidate.contradiction_milli >= request.contradiction_trigger_milli;
    let starved = request
        .now_epoch
        .saturating_sub(candidate.last_refresh_epoch)
        >= request.starvation_bound_epochs;
    let mut reasons = Vec::new();
    if stale {
        reasons.push(ContextRefreshReason::Stale);
    }
    if contradictory {
        reasons.push(ContextRefreshReason::Contradictory);
    }
    if candidate.new_result_count > 0 && dependency_changed {
        reasons.push(ContextRefreshReason::NewResult);
    }
    if starved {
        reasons.push(ContextRefreshReason::Starved);
    }
    if candidate.critical && reasons.is_empty() {
        reasons.push(ContextRefreshReason::PriorityDeclared);
    }
    reasons
}

fn priority(candidate: &ContextRefreshCandidate, reasons: &[ContextRefreshReason]) -> u16 {
    let mut score = u32::from(candidate.base_priority_milli);
    for reason in reasons {
        score = score.saturating_add(match reason {
            ContextRefreshReason::Stale => 180,
            ContextRefreshReason::Contradictory => 260,
            ContextRefreshReason::NewResult => 140,
            ContextRefreshReason::Starved => 220,
            ContextRefreshReason::PriorityDeclared => 120,
        });
    }
    if candidate.critical {
        score = score.saturating_add(180);
    }
    score.min(1_000) as u16
}

fn validate_output(schedule: &ContextRefreshSchedule) -> Result<(), ContextRefreshScheduleError> {
    if schedule.feature_id != FEATURE_ID
        || schedule.output_schema != OUTPUT_SCHEMA
        || !bounded_text(&schedule.objective)
        || schedule.now_epoch == 0
        || !canonical(&schedule.action_order)
        || !canonical(&schedule.deferred_order)
        || schedule.actions.len() != schedule.action_order.len()
        || schedule.deferrals.len() != schedule.deferred_order.len()
        || schedule
            .actions
            .iter()
            .map(|action| action.action_id.clone())
            .collect::<Vec<_>>()
            != schedule.action_order
        || schedule
            .deferrals
            .iter()
            .map(|deferral| deferral.context_id.clone())
            .collect::<Vec<_>>()
            != schedule.deferred_order
        || schedule.actions.iter().any(|action| {
            action.action_id.trim().is_empty()
                || action.context_id.trim().is_empty()
                || action.context_digest.as_str().len() != 64
                || action.fairness_group.trim().is_empty()
                || action.reason_order.is_empty()
                || !canonical(&action.reason_order)
                || action.priority_milli > 1_000
                || action.cost_units == 0
        })
        || schedule.deferrals.iter().any(|deferral| {
            deferral.context_id.trim().is_empty()
                || deferral.fairness_group.trim().is_empty()
                || deferral.priority_milli > 1_000
                || deferral.cost_units == 0
                || deferral.rationale.trim().is_empty()
        })
        || schedule
            .action_order
            .iter()
            .any(|id| schedule.deferred_order.binary_search(id).is_ok())
        || schedule.budget_used_units > schedule.budget_units
        || schedule.budget_remaining_units
            != schedule
                .budget_units
                .saturating_sub(schedule.budget_used_units)
        || schedule
            .uncertainty
            .iter()
            .any(|item| item.trim().is_empty())
        || schedule.next_action.trim().is_empty()
    {
        return Err(ContextRefreshScheduleError::InvalidOutput(
            "identity, action partition, typed reasons, budget accounting, or next action is invalid".into(),
        ));
    }
    let expected = ContentHash::of_value(&digest_input(schedule))
        .map_err(|error| ContextRefreshScheduleError::Digest(error.to_string()))?;
    if expected != schedule.digest {
        return Err(ContextRefreshScheduleError::InvalidOutput(
            "schedule digest is not content-addressed".into(),
        ));
    }
    Ok(())
}

impl ContextRefreshSchedule {
    pub fn validate(&self) -> Result<(), ContextRefreshScheduleError> {
        validate_output(self)
    }
}

#[derive(Debug, Clone)]
struct RankedCandidate {
    candidate: ContextRefreshCandidate,
    reasons: Vec<ContextRefreshReason>,
    priority_milli: u16,
}

fn schedule_one(
    ranked: &RankedCandidate,
    actions: &mut Vec<ContextRefreshAction>,
    selected: &mut BTreeSet<String>,
    budget_used: &mut u64,
    request: &ContextRefreshScheduleRequest,
) -> bool {
    if selected.len() >= request.max_actions
        || budget_used.saturating_add(u64::from(ranked.candidate.cost_units)) > request.budget_units
    {
        return false;
    }
    let action_id = format!("refresh-context-{}", ranked.candidate.context_id);
    selected.insert(ranked.candidate.context_id.clone());
    actions.push(ContextRefreshAction {
        action_id,
        context_id: ranked.candidate.context_id.clone(),
        context_digest: ranked.candidate.context_digest.clone(),
        fairness_group: ranked.candidate.fairness_group.clone(),
        reason_order: ranked.reasons.clone(),
        priority_milli: ranked.priority_milli,
        cost_units: ranked.candidate.cost_units,
        critical: ranked.candidate.critical,
    });
    *budget_used = budget_used.saturating_add(u64::from(ranked.candidate.cost_units));
    true
}

/// Rank and admit context refreshes under hard budget, fairness, and action-count constraints.
pub fn schedule_glioma_context_refresh(
    request: &ContextRefreshScheduleRequest,
) -> Result<ContextRefreshSchedule, ContextRefreshScheduleError> {
    validate_request(request)?;
    if request.candidates.is_empty() {
        let mut schedule = ContextRefreshSchedule {
            feature_id: FEATURE_ID.into(),
            output_schema: OUTPUT_SCHEMA.into(),
            objective: request.objective.clone(),
            now_epoch: request.now_epoch,
            action_order: Vec::new(),
            deferred_order: Vec::new(),
            actions: Vec::new(),
            deferrals: Vec::new(),
            budget_units: request.budget_units,
            budget_used_units: 0,
            budget_remaining_units: request.budget_units,
            disposition: ContextRefreshScheduleDisposition::Unresolved,
            uncertainty: vec!["no context candidates were supplied for refresh".into()],
            next_action: "admit a context candidate or retain the unresolved frontier".into(),
            replay_identity: request.replay_identity.clone(),
            digest: ContentHash::of_bytes(b"unsealed-glioma-context-refresh-schedule"),
        };
        schedule.digest = ContentHash::of_value(&digest_input(&schedule))
            .map_err(|error| ContextRefreshScheduleError::Digest(error.to_string()))?;
        validate_output(&schedule)?;
        return Ok(schedule);
    }
    let mut ranked = request
        .candidates
        .iter()
        .map(|candidate| {
            let reasons = candidate_reasons(candidate, request);
            let priority_milli = priority(candidate, &reasons);
            RankedCandidate {
                candidate: candidate.clone(),
                reasons,
                priority_milli,
            }
        })
        .collect::<Vec<_>>();
    ranked.sort_by(|left, right| {
        right
            .priority_milli
            .cmp(&left.priority_milli)
            .then_with(|| right.candidate.critical.cmp(&left.candidate.critical))
            .then_with(|| left.candidate.context_id.cmp(&right.candidate.context_id))
    });
    let due_ranked = ranked
        .iter()
        .filter(|candidate| !candidate.reasons.is_empty())
        .cloned()
        .collect::<Vec<_>>();
    let mut by_group: BTreeMap<String, Vec<RankedCandidate>> = BTreeMap::new();
    for item in due_ranked.iter().cloned() {
        by_group
            .entry(item.candidate.fairness_group.clone())
            .or_default()
            .push(item);
    }
    let mut actions = Vec::new();
    let mut selected = BTreeSet::new();
    let mut budget_used = 0_u64;
    let mut group_representatives = by_group
        .values()
        .filter_map(|group| group.first())
        .collect::<Vec<_>>();
    group_representatives.sort_by(|left, right| {
        right
            .priority_milli
            .cmp(&left.priority_milli)
            .then_with(|| right.candidate.critical.cmp(&left.candidate.critical))
            .then_with(|| left.candidate.context_id.cmp(&right.candidate.context_id))
    });
    for candidate in group_representatives {
        schedule_one(
            candidate,
            &mut actions,
            &mut selected,
            &mut budget_used,
            request,
        );
    }
    for candidate in &due_ranked {
        if !selected.contains(&candidate.candidate.context_id) {
            schedule_one(
                candidate,
                &mut actions,
                &mut selected,
                &mut budget_used,
                request,
            );
        }
    }
    actions.sort_by(|left, right| left.action_id.cmp(&right.action_id));
    let action_order = actions
        .iter()
        .map(|action| action.action_id.clone())
        .collect::<Vec<_>>();
    let mut deferrals = Vec::new();
    for candidate in &ranked {
        if selected.contains(&candidate.candidate.context_id) {
            continue;
        }
        let reason = if candidate.reasons.is_empty() {
            ContextRefreshDeferralReason::NotDue
        } else if budget_used.saturating_add(u64::from(candidate.candidate.cost_units))
            > request.budget_units
        {
            ContextRefreshDeferralReason::Budget
        } else {
            ContextRefreshDeferralReason::ActionLimit
        };
        let rationale = match reason {
            ContextRefreshDeferralReason::Budget =>
                "hard refresh budget prevented admission; context remains explicitly stale or unresolved".into(),
            ContextRefreshDeferralReason::ActionLimit =>
                "action-count limit deferred this context after higher-priority work".into(),
            ContextRefreshDeferralReason::NotDue =>
                "no stale, contradictory, new-result, starvation, or priority trigger requires a refresh".into(),
        };
        deferrals.push(ContextRefreshDeferral {
            context_id: candidate.candidate.context_id.clone(),
            fairness_group: candidate.candidate.fairness_group.clone(),
            reason,
            priority_milli: candidate.priority_milli,
            cost_units: candidate.candidate.cost_units,
            critical: candidate.candidate.critical,
            rationale,
        });
    }
    deferrals.sort_by(|left, right| left.context_id.cmp(&right.context_id));
    let deferred_order = deferrals
        .iter()
        .map(|deferral| deferral.context_id.clone())
        .collect::<Vec<_>>();
    let critical_budget_blocked = deferrals.iter().any(|deferral| {
        deferral.critical && deferral.reason == ContextRefreshDeferralReason::Budget
    });
    let actionable_deferrals = deferrals
        .iter()
        .any(|deferral| deferral.reason != ContextRefreshDeferralReason::NotDue);
    let disposition = if !actionable_deferrals {
        ContextRefreshScheduleDisposition::Ready
    } else if actions.is_empty() {
        ContextRefreshScheduleDisposition::BudgetBlocked
    } else if deferrals.is_empty() {
        ContextRefreshScheduleDisposition::Ready
    } else if critical_budget_blocked {
        ContextRefreshScheduleDisposition::BudgetBlocked
    } else {
        ContextRefreshScheduleDisposition::Partial
    };
    let mut uncertainty = Vec::new();
    if actionable_deferrals {
        uncertainty
            .push("deferred contexts remain current-state uncertainty until refreshed".into());
    }
    if actions.iter().any(|action| action.reason_order.is_empty()) {
        uncertainty.push(
            "one or more contexts have no refresh trigger and require explicit review".into(),
        );
    }
    let next_action = match disposition {
        ContextRefreshScheduleDisposition::Ready if actions.is_empty() => {
            "retain current contexts and wait for a refresh trigger".into()
        }
        ContextRefreshScheduleDisposition::Ready => {
            "dispatch admitted refresh actions through the local context compiler".into()
        }
        ContextRefreshScheduleDisposition::Partial => {
            "execute admitted refreshes, then revisit explicit action-limit deferrals".into()
        }
        ContextRefreshScheduleDisposition::BudgetBlocked => {
            "increase or reallocate the approved refresh budget before deferring critical contexts"
                .into()
        }
        ContextRefreshScheduleDisposition::Unresolved => {
            "supply a context candidate with a verified dependency epoch".into()
        }
    };
    let mut schedule = ContextRefreshSchedule {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        objective: request.objective.clone(),
        now_epoch: request.now_epoch,
        action_order,
        deferred_order,
        actions,
        deferrals,
        budget_units: request.budget_units,
        budget_used_units: budget_used,
        budget_remaining_units: request.budget_units.saturating_sub(budget_used),
        disposition,
        uncertainty,
        next_action,
        replay_identity: request.replay_identity.clone(),
        digest: ContentHash::of_bytes(b"unsealed-glioma-context-refresh-schedule"),
    };
    schedule.digest = ContentHash::of_value(&digest_input(&schedule))
        .map_err(|error| ContextRefreshScheduleError::Digest(error.to_string()))?;
    validate_output(&schedule)?;
    Ok(schedule)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn candidate(id: &str, group: &str, last: u64, cost: u32) -> ContextRefreshCandidate {
        ContextRefreshCandidate {
            context_id: id.into(),
            context_digest: ContentHash::of_bytes(id.as_bytes()),
            fairness_group: group.into(),
            last_refresh_epoch: last,
            latest_dependency_epoch: last + 1,
            staleness_horizon_epochs: 3,
            contradiction_milli: 0,
            new_result_count: 1,
            base_priority_milli: 500,
            cost_units: cost,
            critical: false,
        }
    }

    fn request() -> ContextRefreshScheduleRequest {
        ContextRefreshScheduleRequest {
            objective: "keep glioma decision contexts current before selecting research actions"
                .into(),
            now_epoch: 10,
            budget_units: 8,
            max_actions: 4,
            contradiction_trigger_milli: 500,
            starvation_bound_epochs: 6,
            candidates: vec![
                candidate("ctx-b", "program-b", 8, 3),
                candidate("ctx-a", "program-a", 1, 3),
                candidate("ctx-c", "program-c", 9, 3),
            ],
            replay_identity: ContentHash::of_bytes(b"refresh-replay"),
        }
    }

    #[test]
    fn schedule_is_deterministic_and_fair_across_groups() {
        let mut first_request = request();
        let first = schedule_glioma_context_refresh(&first_request).unwrap();
        first_request.candidates.reverse();
        let second = schedule_glioma_context_refresh(&first_request).unwrap();
        assert_eq!(first, second);
        assert_eq!(first.actions.len(), 2);
        assert_eq!(first.budget_used_units, 6);
        assert!(first
            .actions
            .iter()
            .any(|action| action.context_id == "ctx-a"));
        first.validate().unwrap();
    }

    #[test]
    fn fresh_noncritical_context_is_not_scheduled_without_refresh_trigger() {
        let input = ContextRefreshScheduleRequest {
            objective: "retain a fresh glioma decision context until evidence changes".into(),
            now_epoch: 10,
            budget_units: 8,
            max_actions: 4,
            contradiction_trigger_milli: 500,
            starvation_bound_epochs: 6,
            candidates: vec![ContextRefreshCandidate {
                context_id: "fresh".into(),
                context_digest: ContentHash::of_bytes(b"fresh"),
                fairness_group: "program".into(),
                last_refresh_epoch: 10,
                latest_dependency_epoch: 10,
                staleness_horizon_epochs: 3,
                contradiction_milli: 0,
                new_result_count: 0,
                base_priority_milli: 500,
                cost_units: 3,
                critical: false,
            }],
            replay_identity: ContentHash::of_bytes(b"fresh-replay"),
        };
        let output = schedule_glioma_context_refresh(&input).unwrap();
        assert!(output.actions.is_empty());
        assert_eq!(output.disposition, ContextRefreshScheduleDisposition::Ready);
        assert_eq!(output.deferred_order, vec!["fresh"]);
        assert_eq!(
            output.deferrals[0].reason,
            ContextRefreshDeferralReason::NotDue
        );
        assert!(output.uncertainty.is_empty());
        assert!(output.next_action.contains("wait"));
        output.validate().unwrap();
    }

    #[test]
    fn contradictory_critical_context_is_prioritized_and_budget_block_is_explicit() {
        let mut input = request();
        input.budget_units = 3;
        input.candidates[0].contradiction_milli = 900;
        input.candidates[0].critical = true;
        input.candidates[0].cost_units = 3;
        let output = schedule_glioma_context_refresh(&input).unwrap();
        assert_eq!(output.actions[0].context_id, "ctx-b");
        assert!(!output.deferrals.is_empty());
        assert_eq!(
            output.disposition,
            ContextRefreshScheduleDisposition::Partial
        );
    }

    #[test]
    fn critical_context_that_cannot_fit_is_not_silently_deferred() {
        let mut input = request();
        input.budget_units = 1;
        input.candidates[0].critical = true;
        input.candidates[0].cost_units = 2;
        let output = schedule_glioma_context_refresh(&input).unwrap();
        assert_eq!(
            output.disposition,
            ContextRefreshScheduleDisposition::BudgetBlocked
        );
        assert!(output
            .deferrals
            .iter()
            .any(|deferral| deferral.context_id == "ctx-b" && deferral.critical));
        assert!(output.next_action.contains("budget"));
    }

    #[test]
    fn empty_candidate_frontier_is_unresolved() {
        let mut input = request();
        input.candidates.clear();
        let output = schedule_glioma_context_refresh(&input).unwrap();
        assert_eq!(
            output.disposition,
            ContextRefreshScheduleDisposition::Unresolved
        );
        assert!(output.next_action.contains("candidate"));
    }
}
