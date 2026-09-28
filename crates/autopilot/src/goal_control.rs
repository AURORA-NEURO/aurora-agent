//! Bounded caller-controlled continuation across multiple missions.
//!
//! Blueprint 40.36 defines how a failed step may be retried; it does not define when a finished
//! mission completes a larger objective or how an agent should plan the next mission. This module
//! keeps that distinction explicit. A caller-owned [`GoalController`] receives the preceding
//! mission report in process and must return a new mission, an evaluator-backed completion claim,
//! or a typed stop. A successful mission is never promoted to completed goal state by this crate.
//!
//! Every mission receives a grant narrowed to the remaining aggregate dispatch allowance. The
//! loop also has a hard mission-cycle ceiling, and it stops immediately when a dispatch outcome is
//! unknown, a policy refusal occurs, or the inner driver pauses. Reports retain mission and report
//! digests, counts, and typed statuses; raw mission documents, tool arguments, mission results,
//! evaluator payloads, and controller error text remain process-local.
//!
//! The control loop is synchronous and caller-driven. [`resume_goal`] accepts a sealed report only
//! after a safe explicit stop and requires the caller to rehydrate the last private mission report.
//! It does not own a clock, planner model, deadline, filesystem, rollback fence, or persistence
//! adapter. Mission-level checkpoint and recovery remain available through [`crate::drive`].

use crate::drive::{drive_mission, MissionDispatch};
use crate::error::{AutopilotError, GrantError};
use crate::grant::{AutonomyGrant, AutonomyGrantDocument};
use crate::report::{verify_autopilot_report, FinalStatus};
use bioprism_ids::ContentHash;
use serde_json::{json, Map, Value};
use std::fmt;
use std::panic::{catch_unwind, AssertUnwindSafe};
use thiserror::Error;

/// Report schema emitted by the goal-level controller.
pub const GOAL_CONTROL_REPORT_SCHEMA: &str = "bioprism-autopilot/goal-control/0.1";
/// Hard ceiling on mission cycles in one in-process control loop.
pub const MAX_GOAL_CONTROL_CYCLES: usize = 128;
/// Maximum aggregate dispatches: each of at most 128 cycles is bounded by the grant's 16 calls.
pub const MAX_GOAL_CONTROL_DISPATCHES: usize =
    MAX_GOAL_CONTROL_CYCLES * crate::grant::MAX_GRANT_ATTEMPTS;

/// Limitations every goal-control report carries.
pub const REQUIRED_GOAL_CONTROL_LIMITATIONS: [&str; 5] = [
    "goal completion is an explicit caller-evaluator assertion and is never inferred from a successful mission",
    "the goal controller receives mission reports in process; this report retains only digests and bounded status metadata",
    "each mission receives a narrowed copy of the same grant and aggregate mission and dispatch ceilings are enforced",
    "resume is allowed only from an explicit safe stop and requires caller-rehydrated report material; uncertain outcomes are terminal",
    "checkpoint adapters offer caller-store compare-and-swap; durable storage, authentication, anti-rollback, and mission recovery remain caller-owned",
];

/// Invalid budgets or identifiers supplied before the controller is invoked.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum GoalControlError {
    #[error("goal id must be a non-empty identifier of at most 256 UTF-8 bytes")]
    InvalidGoalId,
    #[error(
        "goal control budget is invalid: max_cycles must be 1..={maximum_cycles} and max_total_dispatches must be 1..={maximum_dispatches}"
    )]
    InvalidBudget {
        maximum_cycles: usize,
        maximum_dispatches: usize,
    },
    #[error("goal control could not construct a narrowed grant: {0}")]
    Grant(#[from] GrantError),
    #[error("goal control report could not be built: {0}")]
    Autopilot(#[from] AutopilotError),
    #[error("goal control resume was refused: {reason}")]
    InvalidResume { reason: String },
}

/// Per-run ceilings. These are independent of the grant: the grant authorises tools, while this
/// budget prevents repeated mission planning from multiplying that authority without bound.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GoalControlBudget {
    max_cycles: usize,
    max_total_dispatches: usize,
}

impl GoalControlBudget {
    pub fn new(max_cycles: usize, max_total_dispatches: usize) -> Result<Self, GoalControlError> {
        if !(1..=MAX_GOAL_CONTROL_CYCLES).contains(&max_cycles)
            || !(1..=MAX_GOAL_CONTROL_DISPATCHES).contains(&max_total_dispatches)
            || max_total_dispatches > max_cycles.saturating_mul(crate::grant::MAX_GRANT_ATTEMPTS)
        {
            return Err(GoalControlError::InvalidBudget {
                maximum_cycles: MAX_GOAL_CONTROL_CYCLES,
                maximum_dispatches: max_cycles
                    .min(MAX_GOAL_CONTROL_CYCLES)
                    .saturating_mul(crate::grant::MAX_GRANT_ATTEMPTS)
                    .min(MAX_GOAL_CONTROL_DISPATCHES),
            });
        }
        Ok(Self {
            max_cycles,
            max_total_dispatches,
        })
    }

    pub fn max_cycles(self) -> usize {
        self.max_cycles
    }

    pub fn max_total_dispatches(self) -> usize {
        self.max_total_dispatches
    }
}

/// Why a caller-owned controller is stopping without asserting that the goal is complete.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GoalStopReason {
    NoAdmissibleWork,
    NeedsReview,
    Cancelled,
    Abandoned,
}

impl GoalStopReason {
    pub fn as_str(self) -> &'static str {
        match self {
            GoalStopReason::NoAdmissibleWork => "no_admissible_work",
            GoalStopReason::NeedsReview => "needs_review",
            GoalStopReason::Cancelled => "cancelled",
            GoalStopReason::Abandoned => "abandoned",
        }
    }
}

/// The caller's next goal-level transition.
///
/// `Complete` requires a named evaluator and a digest of its evidence. Those fields identify an
/// assertion; they do not make it independently true. `RunMission` carries a private mission only
/// for the current call and is never copied into the control report.
#[derive(Debug, Clone, PartialEq)]
pub enum GoalDecision {
    RunMission(Value),
    Complete {
        evaluator_id: String,
        evidence_sha256: String,
    },
    Stop(GoalStopReason),
}

/// Current read-only view passed to a caller-owned goal controller.
///
/// The last mission report is available only for this synchronous callback. It is not retained
/// by the loop except through its content digest and status projection.
pub struct GoalControlContext<'a> {
    goal_id: &'a str,
    completed_cycles: usize,
    total_dispatches: usize,
    cycle_summaries: &'a [GoalCycleSummary],
    previous_status: Option<FinalStatus>,
    previous_autopilot_report: Option<&'a Value>,
}

impl<'a> GoalControlContext<'a> {
    pub fn goal_id(&self) -> &str {
        self.goal_id
    }

    pub fn completed_cycles(&self) -> usize {
        self.completed_cycles
    }

    pub fn total_dispatches(&self) -> usize {
        self.total_dispatches
    }

    pub fn cycle_summaries(&self) -> &[GoalCycleSummary] {
        self.cycle_summaries
    }

    pub fn previous_status(&self) -> Option<FinalStatus> {
        self.previous_status
    }

    pub fn previous_autopilot_report(&self) -> Option<&Value> {
        self.previous_autopilot_report
    }
}

/// A caller-owned policy/evaluator seam that decides the next mission or terminal transition.
/// A panic while unwinding is enabled becomes a digest-only controller-error stop, preserving
/// every mission cycle already completed.
pub trait GoalController {
    fn decide(&mut self, context: &GoalControlContext<'_>) -> Result<GoalDecision, String>;
}

impl<F> GoalController for F
where
    F: for<'a> FnMut(&GoalControlContext<'a>) -> Result<GoalDecision, String>,
{
    fn decide(&mut self, context: &GoalControlContext<'_>) -> Result<GoalDecision, String> {
        self(context)
    }
}

/// Final state of one bounded goal-level control loop.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GoalControlStatus {
    Completed,
    Stopped,
    CycleBudgetExhausted,
    DispatchBudgetExhausted,
    OutcomeUnknown,
    Refused,
    Paused,
}

impl GoalControlStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            GoalControlStatus::Completed => "completed",
            GoalControlStatus::Stopped => "stopped",
            GoalControlStatus::CycleBudgetExhausted => "cycle_budget_exhausted",
            GoalControlStatus::DispatchBudgetExhausted => "dispatch_budget_exhausted",
            GoalControlStatus::OutcomeUnknown => "outcome_unknown",
            GoalControlStatus::Refused => "refused",
            GoalControlStatus::Paused => "paused",
        }
    }
}

/// Metadata for one mission cycle. The mission and raw report are represented by digests only.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GoalCycleSummary {
    cycle_index: usize,
    mission_sha256: String,
    effective_grant_sha256: String,
    autopilot_report_sha256: Option<String>,
    dispatches_used: usize,
    final_status: FinalStatus,
    drive_error_sha256: Option<String>,
    previous_cycle_sha256: Option<String>,
    cycle_sha256: String,
}

impl GoalCycleSummary {
    pub fn cycle_index(&self) -> usize {
        self.cycle_index
    }

    pub fn mission_sha256(&self) -> &str {
        &self.mission_sha256
    }

    pub fn effective_grant_sha256(&self) -> &str {
        &self.effective_grant_sha256
    }

    pub fn dispatches_used(&self) -> usize {
        self.dispatches_used
    }

    pub fn final_status(&self) -> FinalStatus {
        self.final_status
    }

    pub fn autopilot_report_sha256(&self) -> Option<&str> {
        self.autopilot_report_sha256.as_deref()
    }

    pub fn drive_error_sha256(&self) -> Option<&str> {
        self.drive_error_sha256.as_deref()
    }

    pub fn previous_cycle_sha256(&self) -> Option<&str> {
        self.previous_cycle_sha256.as_deref()
    }

    pub fn cycle_sha256(&self) -> &str {
        &self.cycle_sha256
    }

    fn seal(parts: GoalCycleParts) -> Result<Self, AutopilotError> {
        let mut cycle = Self {
            cycle_index: parts.cycle_index,
            mission_sha256: parts.mission_sha256,
            effective_grant_sha256: parts.effective_grant_sha256,
            autopilot_report_sha256: parts.autopilot_report_sha256,
            dispatches_used: parts.dispatches_used,
            final_status: parts.final_status,
            drive_error_sha256: parts.drive_error_sha256,
            previous_cycle_sha256: parts.previous_cycle_sha256,
            cycle_sha256: String::new(),
        };
        cycle.cycle_sha256 = digest_value(&cycle.unsigned_json())?;
        Ok(cycle)
    }

    fn unsigned_json(&self) -> Value {
        json!({
            "cycle_index": self.cycle_index,
            "mission_sha256": self.mission_sha256,
            "effective_grant_sha256": self.effective_grant_sha256,
            "autopilot_report_sha256": self.autopilot_report_sha256,
            "dispatches_used": self.dispatches_used,
            "final_status": self.final_status.as_str(),
            "drive_error_sha256": self.drive_error_sha256,
            "previous_cycle_sha256": self.previous_cycle_sha256,
        })
    }

    fn to_json(&self) -> Value {
        let mut value = self.unsigned_json();
        value["cycle_sha256"] = Value::String(self.cycle_sha256.clone());
        value
    }
}

struct GoalCycleParts {
    cycle_index: usize,
    mission_sha256: String,
    effective_grant_sha256: String,
    autopilot_report_sha256: Option<String>,
    dispatches_used: usize,
    final_status: FinalStatus,
    drive_error_sha256: Option<String>,
    previous_cycle_sha256: Option<String>,
}

struct GoalControlState {
    cycle_summaries: Vec<GoalCycleSummary>,
    total_dispatches: usize,
    previous_status: Option<FinalStatus>,
    previous_report: Option<Value>,
}

/// Returned process-local cycle reports plus the sealed metadata-only loop report.
#[derive(Debug, Clone, PartialEq)]
pub struct GoalControlOutcome {
    pub final_status: GoalControlStatus,
    pub report: Value,
    /// Number of dispatcher calls made during this invocation, excluding earlier resumed cycles.
    /// This remains available when a call has no returned mission report.
    pub dispatches_this_call: usize,
    /// Raw autopilot reports are returned to the caller, not embedded in the sealed projection.
    /// A caller chooses whether and where to retain these private, process-local documents.
    pub cycle_reports: Vec<Value>,
}

struct CountedDispatcher<'a, D> {
    inner: &'a mut D,
    calls: usize,
}

impl<D: MissionDispatch> MissionDispatch for CountedDispatcher<'_, D> {
    fn dispatch(&mut self, mission: &Value) -> Result<Value, String> {
        self.calls = self.calls.saturating_add(1);
        self.inner.dispatch(mission)
    }
}

fn narrow_grant(
    grant: &AutonomyGrant,
    remaining_dispatches: usize,
) -> Result<AutonomyGrant, GrantError> {
    let mut document = AutonomyGrantDocument::from(grant.clone());
    document.max_attempts = grant.max_attempts().min(remaining_dispatches);
    AutonomyGrant::try_from(document)
}

fn validate_cycle_grant_chain(
    cycles: &[GoalCycleSummary],
    grant: &AutonomyGrant,
    budget: GoalControlBudget,
) -> Result<(), GoalControlError> {
    let mut dispatches_before_cycle = 0_usize;
    for cycle in cycles {
        let remaining = budget
            .max_total_dispatches
            .checked_sub(dispatches_before_cycle)
            .filter(|remaining| *remaining > 0)
            .ok_or_else(|| invalid_resume("checkpoint has a cycle after its dispatch budget"))?;
        let expected = narrow_grant(grant, remaining)?.digest()?;
        if cycle.effective_grant_sha256 != expected {
            return Err(invalid_resume(
                "cycle effective grant does not match the original grant and remaining budget",
            ));
        }
        dispatches_before_cycle = dispatches_before_cycle
            .checked_add(cycle.dispatches_used)
            .ok_or_else(|| invalid_resume("checkpoint dispatch total overflowed"))?;
    }
    Ok(())
}

/// Run a bounded goal-level loop above the existing mission driver.
///
/// Each generated mission is validated and driven under a narrowed copy of `grant`, so the
/// aggregate dispatch total never exceeds `budget.max_total_dispatches()`. A successful mission
/// merely returns control to `controller`; only its explicit `Complete` decision yields
/// `completed`. Unknown outcomes, grant refusals, and pauses are terminal and cannot be bypassed
/// by asking the controller for another mission.
pub fn drive_goal<D, C>(
    goal_id: &str,
    grant: &AutonomyGrant,
    budget: GoalControlBudget,
    controller: &mut C,
    dispatcher: &mut D,
) -> Result<GoalControlOutcome, GoalControlError>
where
    D: MissionDispatch,
    C: GoalController,
{
    drive_goal_from(
        goal_id,
        grant,
        budget,
        GoalControlState {
            cycle_summaries: Vec::new(),
            total_dispatches: 0,
            previous_status: None,
            previous_report: None,
        },
        controller,
        dispatcher,
    )
}

/// Resume a goal from a verified `stopped` goal-control report.
///
/// The report is metadata-only. If it contains a completed mission cycle, the caller must
/// rehydrate the matching private Autopilot report and pass it as `previous_autopilot_report`;
/// this function verifies its digest, validity projection, and status against the last cycle.
/// Resumption preserves the original grant and budget. A report digest is not an authenticity or
/// anti-rollback mechanism, so storage fencing remains caller-owned.
pub fn resume_goal<D, C>(
    checkpoint: &Value,
    previous_autopilot_report: Option<Value>,
    grant: &AutonomyGrant,
    controller: &mut C,
    dispatcher: &mut D,
) -> Result<GoalControlOutcome, GoalControlError>
where
    D: MissionDispatch,
    C: GoalController,
{
    verify_goal_control_report(checkpoint)?;
    if checkpoint["final_status"].as_str() != Some(GoalControlStatus::Stopped.as_str())
        || !matches!(
            checkpoint["disposition"]["reason"].as_str(),
            Some("no_admissible_work" | "needs_review")
        )
    {
        return Err(invalid_resume(
            "only an explicit no_admissible_work or needs_review stop can resume",
        ));
    }

    let goal_id = checkpoint["goal_id"]
        .as_str()
        .ok_or_else(|| invalid_resume("goal_id is missing"))?;
    let grant_sha256 = grant.digest()?;
    if checkpoint["grant_sha256"].as_str() != Some(grant_sha256.as_str()) {
        return Err(invalid_resume(
            "the autonomy grant differs from the checkpoint",
        ));
    }
    let budget_object = checkpoint["budget"]
        .as_object()
        .ok_or_else(|| invalid_resume("budget is missing"))?;
    let max_cycles = required_usize(budget_object, "max_cycles")?;
    let max_total_dispatches = required_usize(budget_object, "max_total_dispatches")?;
    let budget = GoalControlBudget::new(max_cycles, max_total_dispatches)
        .map_err(|error| invalid_resume(&error.to_string()))?;

    let raw_cycles = checkpoint["cycles"]
        .as_array()
        .ok_or_else(|| invalid_resume("cycles are missing"))?;
    let mut cycles = Vec::with_capacity(raw_cycles.len());
    for raw in raw_cycles {
        cycles.push(parse_cycle_summary(raw)?);
    }
    validate_cycle_grant_chain(&cycles, grant, budget)?;
    let total_dispatches = required_usize(
        checkpoint
            .as_object()
            .ok_or_else(|| invalid_resume("checkpoint must be an object"))?,
        "total_dispatches",
    )?;

    let previous_status = cycles.last().map(GoalCycleSummary::final_status);
    let previous_report = match (cycles.last(), previous_autopilot_report) {
        (Some(last_cycle), Some(report)) => {
            if last_cycle.final_status() != FinalStatus::Succeeded
                && last_cycle.final_status() != FinalStatus::Exhausted
            {
                return Err(invalid_resume(
                    "the last mission cycle is not safe for continuation",
                ));
            }
            let expected_digest = last_cycle
                .autopilot_report_sha256()
                .ok_or_else(|| invalid_resume("the last cycle has no Autopilot report"))?;
            let projection = verify_autopilot_report(&report)?;
            let actual_digest = digest_value(&report)?;
            if projection.get("valid").and_then(Value::as_bool) != Some(true)
                || actual_digest != expected_digest
                || report.get("grant_digest").and_then(Value::as_str)
                    != Some(last_cycle.effective_grant_sha256())
                || report.get("final_status").and_then(Value::as_str)
                    != Some(last_cycle.final_status().as_str())
            {
                return Err(invalid_resume(
                    "rehydrated Autopilot report does not match the last goal cycle and its effective grant",
                ));
            }
            Some(report)
        }
        (Some(_), None) => {
            return Err(invalid_resume(
                "rehydrating the last Autopilot report is required to resume",
            ));
        }
        (None, Some(_)) => {
            return Err(invalid_resume(
                "a report cannot be supplied when the checkpoint has no mission cycles",
            ));
        }
        (None, None) => None,
    };

    drive_goal_from(
        goal_id,
        grant,
        budget,
        GoalControlState {
            cycle_summaries: cycles,
            total_dispatches,
            previous_status,
            previous_report,
        },
        controller,
        dispatcher,
    )
}

fn drive_goal_from<D, C>(
    goal_id: &str,
    grant: &AutonomyGrant,
    budget: GoalControlBudget,
    state: GoalControlState,
    controller: &mut C,
    dispatcher: &mut D,
) -> Result<GoalControlOutcome, GoalControlError>
where
    D: MissionDispatch,
    C: GoalController,
{
    let GoalControlState {
        mut cycle_summaries,
        mut total_dispatches,
        mut previous_status,
        mut previous_report,
    } = state;
    let dispatches_at_entry = total_dispatches;
    if !valid_identifier(goal_id, 256) {
        return Err(GoalControlError::InvalidGoalId);
    }

    let grant_sha256 = grant.digest()?;
    let mut cycle_reports = Vec::new();
    let disposition;
    let final_status;

    loop {
        let context = GoalControlContext {
            goal_id,
            completed_cycles: cycle_summaries.len(),
            total_dispatches,
            cycle_summaries: &cycle_summaries,
            previous_status,
            previous_autopilot_report: previous_report.as_ref(),
        };

        let decision_result = match catch_unwind(AssertUnwindSafe(|| controller.decide(&context))) {
            Ok(decision) => decision,
            Err(_) => Err("goal controller panicked before returning a decision".into()),
        };
        let decision = match decision_result {
            Ok(decision) => decision,
            Err(error) => {
                let error_sha256 = digest_text(&error)?;
                final_status = GoalControlStatus::Refused;
                disposition = json!({
                    "kind": "controller_error",
                    "error_sha256": error_sha256,
                });
                break;
            }
        };

        match decision {
            GoalDecision::Complete {
                evaluator_id,
                evidence_sha256,
            } => {
                if cycle_summaries.is_empty()
                    || !valid_identifier(&evaluator_id, 256)
                    || !is_digest(&evidence_sha256)
                {
                    let reason = if cycle_summaries.is_empty() {
                        "completion was declared before any mission produced evidence"
                    } else {
                        "completion needs a bounded evaluator id and lowercase SHA-256 evidence digest"
                    };
                    final_status = GoalControlStatus::Refused;
                    disposition = json!({
                        "kind": "invalid_completion_assertion",
                        "error_sha256": digest_text(reason)?,
                    });
                    break;
                }
                let assessed_cycle_sha256 = cycle_summaries
                    .last()
                    .map(|cycle| cycle.cycle_sha256.clone());
                final_status = GoalControlStatus::Completed;
                disposition = json!({
                    "kind": "evaluator_completed",
                    "evaluator_id": evaluator_id,
                    "evidence_sha256": evidence_sha256,
                    "assessed_cycle_sha256": assessed_cycle_sha256,
                });
                break;
            }
            GoalDecision::Stop(reason) => {
                final_status = GoalControlStatus::Stopped;
                disposition = json!({ "kind": "stopped", "reason": reason.as_str() });
                break;
            }
            GoalDecision::RunMission(mission) => {
                let mission_sha256 = digest_value(&mission)?;
                if cycle_summaries.len() >= budget.max_cycles {
                    final_status = GoalControlStatus::CycleBudgetExhausted;
                    disposition = json!({
                        "kind": "cycle_budget_exhausted",
                        "pending_mission_sha256": mission_sha256,
                    });
                    break;
                }
                if total_dispatches >= budget.max_total_dispatches {
                    final_status = GoalControlStatus::DispatchBudgetExhausted;
                    disposition = json!({
                        "kind": "dispatch_budget_exhausted",
                        "pending_mission_sha256": mission_sha256,
                    });
                    break;
                }

                let remaining = budget.max_total_dispatches - total_dispatches;
                let narrowed_grant = narrow_grant(grant, remaining)?;
                let effective_grant_sha256 = narrowed_grant.digest()?;
                let mut counted = CountedDispatcher {
                    inner: dispatcher,
                    calls: 0,
                };
                let driven = drive_mission(&narrowed_grant, mission.clone(), &mut counted);
                let dispatches_used = counted.calls;
                total_dispatches = total_dispatches.saturating_add(dispatches_used);

                match driven {
                    Ok(outcome) => {
                        let report_sha256 = digest_value(&outcome.report)?;
                        cycle_reports.push(outcome.report.clone());
                        let report_check = verify_autopilot_report(&outcome.report);
                        let report_is_valid = report_check
                            .as_ref()
                            .ok()
                            .and_then(|projection| projection.get("valid"))
                            .and_then(Value::as_bool)
                            == Some(true);
                        let report_attempts = outcome
                            .report
                            .pointer("/totals/attempts_used")
                            .and_then(Value::as_u64)
                            .and_then(|count| usize::try_from(count).ok());
                        let mismatch = !report_is_valid
                            || report_attempts != Some(dispatches_used)
                            || dispatches_used > remaining;
                        let status = if mismatch {
                            FinalStatus::OutcomeUnknown
                        } else {
                            outcome.final_status
                        };
                        let drive_error_sha256 = if mismatch {
                            let reason = report_check
                                .err()
                                .map(|error| error.to_string())
                                .unwrap_or_else(|| {
                                    if !report_is_valid {
                                        "nested autopilot report did not verify as valid".into()
                                    } else {
                                        "nested dispatch accounting does not match actual calls"
                                            .into()
                                    }
                                });
                            Some(digest_text(&reason)?)
                        } else {
                            None
                        };
                        let previous_cycle_sha256 = cycle_summaries
                            .last()
                            .map(|cycle: &GoalCycleSummary| cycle.cycle_sha256.clone());
                        let cycle = GoalCycleSummary::seal(GoalCycleParts {
                            cycle_index: cycle_summaries.len() + 1,
                            mission_sha256,
                            effective_grant_sha256,
                            autopilot_report_sha256: Some(report_sha256),
                            dispatches_used,
                            final_status: status,
                            drive_error_sha256,
                            previous_cycle_sha256,
                        })?;
                        let cycle_sha256 = cycle.cycle_sha256.clone();
                        previous_status = Some(status);
                        previous_report = Some(outcome.report);
                        cycle_summaries.push(cycle);

                        match status {
                            FinalStatus::OutcomeUnknown => {
                                final_status = GoalControlStatus::OutcomeUnknown;
                                disposition = json!({
                                    "kind": "outcome_unknown",
                                    "cycle_sha256": cycle_sha256,
                                });
                                break;
                            }
                            FinalStatus::Refused => {
                                final_status = GoalControlStatus::Refused;
                                disposition = json!({
                                    "kind": "mission_refused",
                                    "cycle_sha256": cycle_sha256,
                                });
                                break;
                            }
                            FinalStatus::Paused => {
                                final_status = GoalControlStatus::Paused;
                                disposition = json!({
                                    "kind": "mission_paused",
                                    "cycle_sha256": cycle_sha256,
                                });
                                break;
                            }
                            FinalStatus::Succeeded | FinalStatus::Exhausted => {}
                        }
                    }
                    Err(error) => {
                        let calls_were_made = dispatches_used > 0;
                        let status = if calls_were_made {
                            FinalStatus::OutcomeUnknown
                        } else {
                            FinalStatus::Refused
                        };
                        let error_sha256 = digest_text(&error.to_string())?;
                        let previous_cycle_sha256 = cycle_summaries
                            .last()
                            .map(|cycle: &GoalCycleSummary| cycle.cycle_sha256.clone());
                        let cycle = GoalCycleSummary::seal(GoalCycleParts {
                            cycle_index: cycle_summaries.len() + 1,
                            mission_sha256,
                            effective_grant_sha256,
                            autopilot_report_sha256: None,
                            dispatches_used,
                            final_status: status,
                            drive_error_sha256: Some(error_sha256.clone()),
                            previous_cycle_sha256,
                        })?;
                        let cycle_sha256 = cycle.cycle_sha256.clone();
                        cycle_summaries.push(cycle);
                        final_status = if calls_were_made {
                            GoalControlStatus::OutcomeUnknown
                        } else {
                            GoalControlStatus::Refused
                        };
                        disposition = json!({
                            "kind": if calls_were_made { "outcome_unknown" } else { "mission_refused" },
                            "cycle_sha256": cycle_sha256,
                            "error_sha256": error_sha256,
                        });
                        break;
                    }
                }
            }
        }
    }

    let report = build_goal_control_report(
        goal_id,
        &grant_sha256,
        budget,
        &cycle_summaries,
        total_dispatches,
        final_status,
        disposition,
    )?;
    Ok(GoalControlOutcome {
        final_status,
        report,
        dispatches_this_call: total_dispatches - dispatches_at_entry,
        cycle_reports,
    })
}

fn build_goal_control_report(
    goal_id: &str,
    grant_sha256: &str,
    budget: GoalControlBudget,
    cycles: &[GoalCycleSummary],
    total_dispatches: usize,
    final_status: GoalControlStatus,
    disposition: Value,
) -> Result<Value, GoalControlError> {
    let mut report = json!({
        "schema": GOAL_CONTROL_REPORT_SCHEMA,
        "goal_id": goal_id,
        "grant_sha256": grant_sha256,
        "budget": {
            "max_cycles": budget.max_cycles,
            "max_total_dispatches": budget.max_total_dispatches,
        },
        "cycles": cycles.iter().map(GoalCycleSummary::to_json).collect::<Vec<_>>(),
        "total_dispatches": total_dispatches,
        "final_status": final_status.as_str(),
        "disposition": disposition,
        "limitations": REQUIRED_GOAL_CONTROL_LIMITATIONS,
    });
    report["report_sha256"] = Value::String(digest_value(&report)?);
    verify_goal_control_report(&report)?;
    Ok(report)
}

/// Verify the goal-control digest, cycle chain, aggregate budget, completion assertion, and
/// mandatory limitations. The returned projection is safe to publish separately from cycle
/// reports, which can contain process-local execution details.
pub fn verify_goal_control_report(report: &Value) -> Result<Value, GoalControlError> {
    let object = report
        .as_object()
        .ok_or_else(|| invalid_report("report must be a JSON object"))?;
    let fields = [
        "schema",
        "goal_id",
        "grant_sha256",
        "budget",
        "cycles",
        "total_dispatches",
        "final_status",
        "disposition",
        "limitations",
        "report_sha256",
    ];
    reject_unknown_fields(object, "goal-control report", &fields)?;
    if object.get("schema").and_then(Value::as_str) != Some(GOAL_CONTROL_REPORT_SCHEMA) {
        return Err(invalid_report("schema version is missing or unsupported"));
    }
    let goal_id = object
        .get("goal_id")
        .and_then(Value::as_str)
        .ok_or_else(|| invalid_report("goal_id must be a string"))?;
    if !valid_identifier(goal_id, 256) {
        return Err(invalid_report("goal_id is malformed"));
    }
    let grant_sha256 = object
        .get("grant_sha256")
        .and_then(Value::as_str)
        .ok_or_else(|| invalid_report("grant_sha256 must be a string"))?;
    if !is_digest(grant_sha256) {
        return Err(invalid_report("grant_sha256 is malformed"));
    }
    let budget = object
        .get("budget")
        .and_then(Value::as_object)
        .ok_or_else(|| invalid_report("budget must be an object"))?;
    reject_unknown_fields(budget, "budget", &["max_cycles", "max_total_dispatches"])?;
    let max_cycles = required_usize(budget, "max_cycles")?;
    let max_total_dispatches = required_usize(budget, "max_total_dispatches")?;
    let budget = GoalControlBudget::new(max_cycles, max_total_dispatches)
        .map_err(|error| invalid_report(&error.to_string()))?;

    let cycles = object
        .get("cycles")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid_report("cycles must be an array"))?;
    if cycles.len() > budget.max_cycles {
        return Err(invalid_report("cycles exceeds max_cycles"));
    }
    let mut expected_previous: Option<String> = None;
    let mut dispatch_total = 0_usize;
    for (index, raw) in cycles.iter().enumerate() {
        let cycle = raw
            .as_object()
            .ok_or_else(|| invalid_report("every cycle must be an object"))?;
        let cycle_fields = [
            "cycle_index",
            "mission_sha256",
            "effective_grant_sha256",
            "autopilot_report_sha256",
            "dispatches_used",
            "final_status",
            "drive_error_sha256",
            "previous_cycle_sha256",
            "cycle_sha256",
        ];
        reject_unknown_fields(cycle, "goal-control cycle", &cycle_fields)?;
        require_fields(cycle, "goal-control cycle", &cycle_fields)?;
        if required_usize(cycle, "cycle_index")? != index + 1 {
            return Err(invalid_report("cycle indexes are not contiguous"));
        }
        for field in ["mission_sha256", "effective_grant_sha256", "cycle_sha256"] {
            if !cycle
                .get(field)
                .and_then(Value::as_str)
                .is_some_and(is_digest)
            {
                return Err(invalid_report(&format!("{field} is malformed")));
            }
        }
        let reported_previous = optional_digest(cycle, "previous_cycle_sha256")?;
        if reported_previous != expected_previous {
            return Err(invalid_report("cycle digest chain is broken"));
        }
        let report_digest = optional_digest(cycle, "autopilot_report_sha256")?;
        let error_digest = optional_digest(cycle, "drive_error_sha256")?;
        if report_digest.is_none() && error_digest.is_none() {
            return Err(invalid_report(
                "a cycle must bind an autopilot report or a drive error",
            ));
        }
        let dispatches = required_usize(cycle, "dispatches_used")?;
        if dispatches > crate::grant::MAX_GRANT_ATTEMPTS {
            return Err(invalid_report(
                "one cycle exceeds the per-mission dispatch ceiling",
            ));
        }
        dispatch_total = dispatch_total
            .checked_add(dispatches)
            .ok_or_else(|| invalid_report("dispatch total overflowed"))?;
        let status = cycle
            .get("final_status")
            .and_then(Value::as_str)
            .ok_or_else(|| invalid_report("final_status must be a string"))?;
        if !FinalStatus::ALL
            .iter()
            .any(|known| known.as_str() == status)
        {
            return Err(invalid_report("cycle final_status is unknown"));
        }
        match (
            report_digest.is_some(),
            error_digest.is_some(),
            status,
            dispatches,
        ) {
            (false, false, _, _) => {
                return Err(invalid_report(
                    "a cycle must bind an autopilot report or a drive error",
                ));
            }
            (false, true, "refused", 0) | (false, true, "outcome_unknown", 1..=16) => {}
            (false, true, _, _) => {
                return Err(invalid_report(
                    "a drive-error cycle has an inconsistent status or dispatch count",
                ));
            }
            (true, true, "outcome_unknown", _) => {}
            (true, true, _, _) => {
                return Err(invalid_report(
                    "a report-integrity error must make the cycle outcome unknown",
                ));
            }
            (true, false, _, _) => {}
        }
        let mut unsigned = Value::Object(cycle.clone());
        unsigned
            .as_object_mut()
            .expect("cycle was an object")
            .remove("cycle_sha256");
        let actual = digest_value(&unsigned)?;
        if cycle.get("cycle_sha256").and_then(Value::as_str) != Some(actual.as_str()) {
            return Err(invalid_report("cycle digest does not match its contents"));
        }
        expected_previous = Some(actual);
    }
    if dispatch_total != required_usize(object, "total_dispatches")? {
        return Err(invalid_report(
            "total_dispatches does not equal the cycle sum",
        ));
    }
    if dispatch_total > budget.max_total_dispatches {
        return Err(invalid_report("cycles exceed max_total_dispatches"));
    }

    let final_status = object
        .get("final_status")
        .and_then(Value::as_str)
        .ok_or_else(|| invalid_report("final_status must be a string"))?;
    let disposition = object
        .get("disposition")
        .and_then(Value::as_object)
        .ok_or_else(|| invalid_report("disposition must be an object"))?;
    let disposition_kind = disposition
        .get("kind")
        .and_then(Value::as_str)
        .ok_or_else(|| invalid_report("disposition.kind must be a string"))?;
    let disposition_fields: &[&str] = match disposition_kind {
        "evaluator_completed" => &[
            "kind",
            "evaluator_id",
            "evidence_sha256",
            "assessed_cycle_sha256",
        ],
        "stopped" => &["kind", "reason"],
        "cycle_budget_exhausted" | "dispatch_budget_exhausted" => {
            &["kind", "pending_mission_sha256"]
        }
        "outcome_unknown" | "mission_refused" | "mission_paused" => {
            &["kind", "cycle_sha256", "error_sha256"]
        }
        "controller_error" | "invalid_completion_assertion" => &["kind", "error_sha256"],
        _ => &["kind"],
    };
    reject_unknown_fields(disposition, "disposition", disposition_fields)?;
    require_fields(disposition, "disposition", &["kind"])?;
    if disposition.contains_key("error_sha256")
        && !disposition
            .get("error_sha256")
            .and_then(Value::as_str)
            .is_some_and(is_digest)
    {
        return Err(invalid_report("disposition.error_sha256 is malformed"));
    }
    let status_matches = match disposition_kind {
        "evaluator_completed" => {
            final_status == GoalControlStatus::Completed.as_str()
                && !cycles.is_empty()
                && cycles_are_continuable(cycles)
                && disposition
                    .get("evaluator_id")
                    .and_then(Value::as_str)
                    .is_some_and(|id| valid_identifier(id, 256))
                && disposition
                    .get("evidence_sha256")
                    .and_then(Value::as_str)
                    .is_some_and(is_digest)
                && disposition
                    .get("assessed_cycle_sha256")
                    .and_then(Value::as_str)
                    == expected_previous.as_deref()
        }
        "stopped" => {
            final_status == GoalControlStatus::Stopped.as_str()
                && cycles_are_continuable(cycles)
                && matches!(
                    disposition.get("reason").and_then(Value::as_str),
                    Some("no_admissible_work" | "needs_review" | "cancelled" | "abandoned")
                )
        }
        "cycle_budget_exhausted" => {
            final_status == GoalControlStatus::CycleBudgetExhausted.as_str()
                && cycles.len() == budget.max_cycles
                && cycles_are_continuable(cycles)
                && disposition
                    .get("pending_mission_sha256")
                    .and_then(Value::as_str)
                    .is_some_and(is_digest)
        }
        "dispatch_budget_exhausted" => {
            final_status == GoalControlStatus::DispatchBudgetExhausted.as_str()
                && dispatch_total == budget.max_total_dispatches
                && cycles_are_continuable(cycles)
                && disposition
                    .get("pending_mission_sha256")
                    .and_then(Value::as_str)
                    .is_some_and(is_digest)
        }
        "outcome_unknown" => {
            final_status == GoalControlStatus::OutcomeUnknown.as_str()
                && prior_cycles_are_continuable(cycles)
                && disposition.get("cycle_sha256").and_then(Value::as_str)
                    == expected_previous.as_deref()
                && cycles
                    .last()
                    .and_then(Value::as_object)
                    .and_then(|cycle| cycle.get("final_status"))
                    .and_then(Value::as_str)
                    == Some(FinalStatus::OutcomeUnknown.as_str())
        }
        "mission_refused" => {
            final_status == GoalControlStatus::Refused.as_str()
                && prior_cycles_are_continuable(cycles)
                && disposition.get("cycle_sha256").and_then(Value::as_str)
                    == expected_previous.as_deref()
                && cycles
                    .last()
                    .and_then(Value::as_object)
                    .and_then(|cycle| cycle.get("final_status"))
                    .and_then(Value::as_str)
                    == Some(FinalStatus::Refused.as_str())
        }
        "mission_paused" => {
            final_status == GoalControlStatus::Paused.as_str()
                && prior_cycles_are_continuable(cycles)
                && disposition.get("cycle_sha256").and_then(Value::as_str)
                    == expected_previous.as_deref()
                && cycles
                    .last()
                    .and_then(Value::as_object)
                    .and_then(|cycle| cycle.get("final_status"))
                    .and_then(Value::as_str)
                    == Some(FinalStatus::Paused.as_str())
        }
        "controller_error" | "invalid_completion_assertion" => {
            final_status == GoalControlStatus::Refused.as_str()
                && cycles_are_continuable(cycles)
                && disposition
                    .get("error_sha256")
                    .and_then(Value::as_str)
                    .is_some_and(is_digest)
        }
        _ => false,
    };
    if !status_matches {
        return Err(invalid_report(
            "final_status and disposition contradict each other",
        ));
    }

    let limitations = object
        .get("limitations")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid_report("limitations must be an array"))?;
    for required in REQUIRED_GOAL_CONTROL_LIMITATIONS {
        if !limitations
            .iter()
            .any(|line| line.as_str() == Some(required))
        {
            return Err(invalid_report(&format!(
                "required limitation is missing: {required}"
            )));
        }
    }
    let claimed_digest = object
        .get("report_sha256")
        .and_then(Value::as_str)
        .filter(|digest| is_digest(digest))
        .ok_or_else(|| invalid_report("report_sha256 is malformed"))?;
    let mut unsigned = report.clone();
    unsigned
        .as_object_mut()
        .expect("report was an object")
        .remove("report_sha256");
    let actual_digest = digest_value(&unsigned)?;
    if claimed_digest != actual_digest {
        return Err(invalid_report("report digest does not match its contents"));
    }

    Ok(json!({
        "valid": true,
        "schema": GOAL_CONTROL_REPORT_SCHEMA,
        "goal_id": goal_id,
        "final_status": final_status,
        "cycles": cycles.len(),
        "total_dispatches": dispatch_total,
        "report_sha256": actual_digest,
        "limitations_present": true,
    }))
}

fn parse_cycle_summary(value: &Value) -> Result<GoalCycleSummary, GoalControlError> {
    let object = value
        .as_object()
        .ok_or_else(|| invalid_resume("cycle entry must be an object"))?;
    let final_status = match object.get("final_status").and_then(Value::as_str) {
        Some("succeeded") => FinalStatus::Succeeded,
        Some("exhausted") => FinalStatus::Exhausted,
        Some("outcome_unknown") => FinalStatus::OutcomeUnknown,
        Some("refused") => FinalStatus::Refused,
        Some("paused") => FinalStatus::Paused,
        _ => return Err(invalid_resume("cycle final_status is unknown")),
    };
    Ok(GoalCycleSummary {
        cycle_index: required_usize(object, "cycle_index")?,
        mission_sha256: required_string(object, "mission_sha256")?,
        effective_grant_sha256: required_string(object, "effective_grant_sha256")?,
        autopilot_report_sha256: optional_digest(object, "autopilot_report_sha256")?,
        dispatches_used: required_usize(object, "dispatches_used")?,
        final_status,
        drive_error_sha256: optional_digest(object, "drive_error_sha256")?,
        previous_cycle_sha256: optional_digest(object, "previous_cycle_sha256")?,
        cycle_sha256: required_string(object, "cycle_sha256")?,
    })
}

fn valid_identifier(value: &str, maximum_bytes: usize) -> bool {
    !value.trim().is_empty()
        && value == value.trim()
        && !value.contains('\0')
        && value.len() <= maximum_bytes
}

fn cycles_are_continuable(cycles: &[Value]) -> bool {
    cycles.iter().all(|cycle| {
        matches!(
            cycle.get("final_status").and_then(Value::as_str),
            Some("succeeded" | "exhausted")
        )
    })
}

fn prior_cycles_are_continuable(cycles: &[Value]) -> bool {
    !cycles.is_empty() && cycles_are_continuable(&cycles[..cycles.len() - 1])
}

fn is_digest(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn digest_value(value: &Value) -> Result<String, AutopilotError> {
    ContentHash::of_value(value)
        .map(|digest| digest.to_string())
        .map_err(|error| AutopilotError::Canonicalisation {
            reason: error.to_string(),
        })
}

fn digest_text(value: &str) -> Result<String, AutopilotError> {
    digest_value(&Value::String(value.to_string()))
}

fn reject_unknown_fields(
    object: &Map<String, Value>,
    what: &str,
    declared: &[&str],
) -> Result<(), GoalControlError> {
    if let Some(unknown) = object.keys().find(|key| !declared.contains(&key.as_str())) {
        return Err(invalid_report(&format!(
            "undeclared field {unknown:?} on {what}"
        )));
    }
    Ok(())
}

fn require_fields(
    object: &Map<String, Value>,
    what: &str,
    required: &[&str],
) -> Result<(), GoalControlError> {
    if let Some(missing) = required.iter().find(|field| !object.contains_key(**field)) {
        return Err(invalid_report(&format!(
            "required field {missing:?} is missing from {what}"
        )));
    }
    Ok(())
}

fn required_usize(object: &Map<String, Value>, field: &str) -> Result<usize, GoalControlError> {
    object
        .get(field)
        .and_then(Value::as_u64)
        .and_then(|value| usize::try_from(value).ok())
        .ok_or_else(|| invalid_report(&format!("{field} must be a non-negative integer")))
}

fn required_string(object: &Map<String, Value>, field: &str) -> Result<String, GoalControlError> {
    object
        .get(field)
        .and_then(Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| invalid_resume(&format!("{field} must be a string")))
}

fn optional_digest(
    object: &Map<String, Value>,
    field: &str,
) -> Result<Option<String>, GoalControlError> {
    match object.get(field) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(value)) if is_digest(value) => Ok(Some(value.clone())),
        _ => Err(invalid_report(&format!(
            "{field} must be null or a lowercase SHA-256 digest"
        ))),
    }
}

fn invalid_report(reason: &str) -> GoalControlError {
    GoalControlError::Autopilot(AutopilotError::InvalidAutopilotReport {
        reason: format!("invalid goal-control report: {reason}"),
    })
}

fn invalid_resume(reason: &str) -> GoalControlError {
    GoalControlError::InvalidResume {
        reason: reason.to_string(),
    }
}

impl fmt::Display for GoalControlStatus {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bioprism_devplat::{
        plan_mission, MissionReport, MissionRequest, MissionStepResult, MISSION_SCHEMA_VERSION,
        MISSION_TRACE_SCHEMA_VERSION,
    };

    fn grant() -> AutonomyGrant {
        serde_json::from_value(json!({
            "allowed_tools": ["observe"],
            "max_attempts": 4,
            "require_reconciliation_complete": false,
        }))
        .expect("test grant is valid")
    }

    fn reconciliation_required_grant() -> AutonomyGrant {
        serde_json::from_value(json!({
            "allowed_tools": ["observe"],
            "max_attempts": 4,
        }))
        .expect("test grant with default reconciliation requirement is valid")
    }

    fn reseal(value: &mut Value, digest_field: &str) {
        value
            .as_object_mut()
            .expect("sealed value is an object")
            .remove(digest_field);
        value[digest_field] = Value::String(digest_value(value).unwrap());
    }

    fn mission(id: &str) -> Value {
        json!({
            "mission_id": id,
            "goal": "complete one bounded evidence step",
            "steps": [{
                "id": "step-1",
                "domain": "metrics",
                "capability": "analytics",
                "objective": "observe the declared fixture",
                "tool": "observe",
                "arguments": {},
                "depends_on": [],
                "bindings": [],
                "required": true,
            }],
        })
    }

    fn report_for(mission: &Value) -> Value {
        let request: MissionRequest = serde_json::from_value(mission.clone()).unwrap();
        let plan = plan_mission(&request).unwrap();
        let result = MissionStepResult {
            id: "step-1".into(),
            tool: "observe".into(),
            status: "succeeded".into(),
            required: true,
            arguments_digest: Some("1".repeat(64)),
            bytes: 2,
            wire: Some(json!({
                "jsonrpc": "2.0",
                "id": "step-1",
                "result": { "content": [{ "type": "text", "text": "{}" }] },
            })),
            error: None,
        };
        let claim_lineage = bioprism_devplat::mission_claim_lineage_with_review(
            &request.claim_requests,
            std::slice::from_ref(&result),
            request.evaluator_review.as_ref(),
        );
        serde_json::to_value(MissionReport {
            schema_version: MISSION_SCHEMA_VERSION.into(),
            plan,
            execution: "executed".into(),
            mission_status: "succeeded".into(),
            succeeded: 1,
            refused: 0,
            blocked: 0,
            cancelled: 0,
            required_failures: 0,
            returned_bytes: 2,
            results: vec![result],
            execution_trace_schema_version: MISSION_TRACE_SCHEMA_VERSION.into(),
            execution_trace: Vec::new(),
            claim_requests: request.claim_requests.clone(),
            evaluator_review: request.evaluator_review.clone(),
            claim_lineage,
            trace_observer: None,
            guarantees: Vec::new(),
            limitations: Vec::new(),
        })
        .unwrap()
    }

    #[derive(Default)]
    struct FakeDispatcher {
        calls: usize,
        fail: bool,
    }

    impl MissionDispatch for FakeDispatcher {
        fn dispatch(&mut self, mission: &Value) -> Result<Value, String> {
            self.calls += 1;
            if self.fail {
                Err("private transport detail".into())
            } else {
                Ok(report_for(mission))
            }
        }
    }

    #[test]
    fn only_a_named_evaluator_completion_ends_a_multi_mission_goal_as_complete() {
        let mut mission_number = 0;
        let mut controller = |context: &GoalControlContext<'_>| {
            if context.completed_cycles() < 2 {
                mission_number += 1;
                Ok(GoalDecision::RunMission(mission(&format!(
                    "private-mission-{mission_number}"
                ))))
            } else {
                Ok(GoalDecision::Complete {
                    evaluator_id: "goal-evaluator-v1".into(),
                    evidence_sha256: "a".repeat(64),
                })
            }
        };
        let mut dispatcher = FakeDispatcher::default();
        let outcome = drive_goal(
            "goal-7",
            &grant(),
            GoalControlBudget::new(3, 8).unwrap(),
            &mut controller,
            &mut dispatcher,
        )
        .unwrap();

        assert_eq!(outcome.final_status, GoalControlStatus::Completed);
        assert_eq!(outcome.dispatches_this_call, 2);
        assert_eq!(dispatcher.calls, 2);
        assert_eq!(outcome.cycle_reports.len(), 2);
        assert_eq!(outcome.report["cycles"].as_array().unwrap().len(), 2);
        assert_eq!(
            outcome.report["disposition"]["kind"],
            Value::from("evaluator_completed")
        );
        assert_eq!(
            verify_goal_control_report(&outcome.report).unwrap()["valid"],
            Value::Bool(true)
        );
        let report_text = outcome.report.to_string();
        assert!(!report_text.contains("private-mission-"));
        assert!(report_text.contains("evidence_sha256"));
    }

    #[test]
    fn a_pre_dispatch_exhaustion_is_a_valid_zero_dispatch_cycle() {
        let mut controller = |context: &GoalControlContext<'_>| {
            if context.completed_cycles() == 0 {
                Ok(GoalDecision::RunMission(mission(
                    "missing-reconciliation-binding",
                )))
            } else {
                Ok(GoalDecision::Stop(GoalStopReason::NeedsReview))
            }
        };
        let mut dispatcher = FakeDispatcher::default();
        let outcome = drive_goal(
            "goal-preflight-refusal",
            &reconciliation_required_grant(),
            GoalControlBudget::new(2, 8).unwrap(),
            &mut controller,
            &mut dispatcher,
        )
        .unwrap();

        assert_eq!(dispatcher.calls, 0);
        assert_eq!(outcome.final_status, GoalControlStatus::Stopped);
        assert_eq!(
            outcome.report["cycles"][0]["dispatches_used"],
            Value::from(0)
        );
        assert_eq!(
            outcome.report["cycles"][0]["final_status"],
            Value::from("exhausted")
        );
        verify_goal_control_report(&outcome.report).expect("pre-dispatch exhaustion verifies");
    }

    #[test]
    fn a_safe_stop_resumes_from_the_sealed_cycle_with_matching_rehydrated_report() {
        let mut first_controller = |context: &GoalControlContext<'_>| {
            if context.completed_cycles() == 0 {
                Ok(GoalDecision::RunMission(mission("resumable-mission")))
            } else {
                Ok(GoalDecision::Stop(GoalStopReason::NoAdmissibleWork))
            }
        };
        let mut dispatcher = FakeDispatcher::default();
        let stopped = drive_goal(
            "goal-resume",
            &grant(),
            GoalControlBudget::new(4, 12).unwrap(),
            &mut first_controller,
            &mut dispatcher,
        )
        .unwrap();
        assert_eq!(stopped.dispatches_this_call, 1);
        let rehydrated_report = stopped.cycle_reports.last().cloned();
        let mut resumed_controller = |context: &GoalControlContext<'_>| {
            assert_eq!(context.completed_cycles(), 1);
            assert_eq!(context.total_dispatches(), 1);
            assert_eq!(context.previous_status(), Some(FinalStatus::Succeeded));
            assert_eq!(
                context
                    .previous_autopilot_report()
                    .and_then(|report| report.get("final_status"))
                    .and_then(Value::as_str),
                Some("succeeded")
            );
            Ok(GoalDecision::Complete {
                evaluator_id: "resume-evaluator-v1".into(),
                evidence_sha256: "c".repeat(64),
            })
        };
        let resumed = resume_goal(
            &stopped.report,
            rehydrated_report,
            &grant(),
            &mut resumed_controller,
            &mut dispatcher,
        )
        .unwrap();

        assert_eq!(resumed.final_status, GoalControlStatus::Completed);
        assert_eq!(resumed.dispatches_this_call, 0);
        assert_eq!(
            dispatcher.calls, 1,
            "resume must not replay the completed cycle"
        );
        assert_eq!(resumed.report["cycles"].as_array().unwrap().len(), 1);
        assert_eq!(resumed.report["total_dispatches"], Value::from(1));
        verify_goal_control_report(&resumed.report).expect("resumed report verifies");
    }

    #[test]
    fn resume_refuses_missing_or_mismatched_private_report_rehydration() {
        let mut controller = |context: &GoalControlContext<'_>| {
            if context.completed_cycles() == 0 {
                Ok(GoalDecision::RunMission(mission("resume-binding-mission")))
            } else {
                Ok(GoalDecision::Stop(GoalStopReason::NeedsReview))
            }
        };
        let mut dispatcher = FakeDispatcher::default();
        let stopped = drive_goal(
            "goal-resume-binding",
            &grant(),
            GoalControlBudget::new(3, 8).unwrap(),
            &mut controller,
            &mut dispatcher,
        )
        .unwrap();
        let mut resume_controller =
            |_: &GoalControlContext<'_>| Ok(GoalDecision::Stop(GoalStopReason::NoAdmissibleWork));

        assert!(resume_goal(
            &stopped.report,
            None,
            &grant(),
            &mut resume_controller,
            &mut dispatcher,
        )
        .is_err());
        let mut wrong_report = stopped.cycle_reports[0].clone();
        wrong_report["final_status"] = Value::String("exhausted".into());
        assert!(resume_goal(
            &stopped.report,
            Some(wrong_report),
            &grant(),
            &mut resume_controller,
            &mut dispatcher,
        )
        .is_err());
        assert_eq!(dispatcher.calls, 1);
    }

    #[test]
    fn resume_rejects_a_restamped_private_report_from_a_different_effective_grant() {
        let mut controller = |context: &GoalControlContext<'_>| {
            if context.completed_cycles() == 0 {
                Ok(GoalDecision::RunMission(mission("grant-binding-mission")))
            } else {
                Ok(GoalDecision::Stop(GoalStopReason::NeedsReview))
            }
        };
        let mut dispatcher = FakeDispatcher::default();
        let stopped = drive_goal(
            "goal-grant-binding",
            &grant(),
            GoalControlBudget::new(3, 8).unwrap(),
            &mut controller,
            &mut dispatcher,
        )
        .unwrap();

        let mut other_grant_document = AutonomyGrantDocument::from(grant());
        other_grant_document.max_attempts = 5;
        let other_grant = AutonomyGrant::try_from(other_grant_document).unwrap();
        let mut foreign_report = stopped.cycle_reports[0].clone();
        foreign_report["grant"] =
            serde_json::to_value(AutonomyGrantDocument::from(other_grant.clone())).unwrap();
        foreign_report["grant_digest"] = Value::String(other_grant.digest().unwrap());
        foreign_report["totals"]["max_attempts"] = Value::from(5);
        reseal(&mut foreign_report, "report_sha256");

        let mut checkpoint = stopped.report.clone();
        let mut cycle = checkpoint["cycles"][0].clone();
        cycle["autopilot_report_sha256"] = Value::String(digest_value(&foreign_report).unwrap());
        reseal(&mut cycle, "cycle_sha256");
        checkpoint["cycles"][0] = cycle;
        reseal(&mut checkpoint, "report_sha256");
        verify_goal_control_report(&checkpoint)
            .expect("the digest-only report remains structurally self-consistent");

        let mut resume_controller =
            |_: &GoalControlContext<'_>| Ok(GoalDecision::Stop(GoalStopReason::NoAdmissibleWork));
        assert!(matches!(
            resume_goal(
                &checkpoint,
                Some(foreign_report),
                &grant(),
                &mut resume_controller,
                &mut dispatcher,
            ),
            Err(GoalControlError::InvalidResume { reason }) if reason.contains("effective grant")
        ));
        assert_eq!(dispatcher.calls, 1, "rejected rehydration never dispatches");

        let mut foreign_cycle = stopped.report.clone();
        foreign_cycle["cycles"][0]["effective_grant_sha256"] = Value::String("e".repeat(64));
        let mut cycle = foreign_cycle["cycles"][0].clone();
        reseal(&mut cycle, "cycle_sha256");
        foreign_cycle["cycles"][0] = cycle;
        reseal(&mut foreign_cycle, "report_sha256");
        assert!(matches!(
            resume_goal(
                &foreign_cycle,
                Some(stopped.cycle_reports[0].clone()),
                &grant(),
                &mut resume_controller,
                &mut dispatcher,
            ),
            Err(GoalControlError::InvalidResume { reason }) if reason.contains("cycle effective grant")
        ));
        assert_eq!(
            dispatcher.calls, 1,
            "a restamped foreign grant is never dispatched"
        );
    }

    #[test]
    fn an_ambiguous_dispatch_checkpoint_cannot_be_resumed_or_replayed() {
        let mut controller = |_: &GoalControlContext<'_>| {
            Ok(GoalDecision::RunMission(mission(
                "ambiguous-resume-mission",
            )))
        };
        let mut dispatcher = FakeDispatcher {
            calls: 0,
            fail: true,
        };
        let outcome = drive_goal(
            "goal-ambiguous-resume",
            &grant(),
            GoalControlBudget::new(3, 8).unwrap(),
            &mut controller,
            &mut dispatcher,
        )
        .unwrap();
        let mut resume_controller = |_: &GoalControlContext<'_>| {
            Ok(GoalDecision::RunMission(mission("must-not-be-dispatched")))
        };

        assert_eq!(outcome.final_status, GoalControlStatus::OutcomeUnknown);
        assert!(resume_goal(
            &outcome.report,
            outcome.cycle_reports.first().cloned(),
            &grant(),
            &mut resume_controller,
            &mut dispatcher,
        )
        .is_err());
        assert_eq!(
            dispatcher.calls, 1,
            "uncertain external work must not replay"
        );
    }

    #[test]
    fn goal_budget_rejects_empty_and_per_cycle_impossible_dispatch_ceiling() {
        assert!(GoalControlBudget::new(0, 1).is_err());
        assert!(GoalControlBudget::new(1, crate::grant::MAX_GRANT_ATTEMPTS + 1).is_err());
        assert!(GoalControlBudget::new(2, crate::grant::MAX_GRANT_ATTEMPTS * 2 + 1).is_err());
        assert_eq!(
            GoalControlBudget::new(MAX_GOAL_CONTROL_CYCLES, MAX_GOAL_CONTROL_DISPATCHES)
                .unwrap()
                .max_total_dispatches(),
            MAX_GOAL_CONTROL_DISPATCHES
        );
    }

    #[test]
    fn a_restamped_report_still_rejects_an_undeclared_disposition_field() {
        let mut controller =
            |_: &GoalControlContext<'_>| Ok(GoalDecision::Stop(GoalStopReason::NoAdmissibleWork));
        let mut dispatcher = FakeDispatcher::default();
        let outcome = drive_goal(
            "goal-strict-disposition",
            &grant(),
            GoalControlBudget::new(1, 4).unwrap(),
            &mut controller,
            &mut dispatcher,
        )
        .unwrap();
        let mut tampered = outcome.report;
        tampered["disposition"]["unreviewed"] = Value::Bool(true);
        let mut unsigned = tampered.clone();
        unsigned
            .as_object_mut()
            .expect("report is an object")
            .remove("report_sha256");
        tampered["report_sha256"] = Value::String(digest_value(&unsigned).unwrap());

        assert!(verify_goal_control_report(&tampered).is_err());
    }

    #[test]
    fn a_successful_mission_does_not_complete_the_goal_without_the_evaluator_transition() {
        let mut controller = |context: &GoalControlContext<'_>| {
            if context.completed_cycles() == 0 {
                Ok(GoalDecision::RunMission(mission("single-private-mission")))
            } else {
                Ok(GoalDecision::Stop(GoalStopReason::NeedsReview))
            }
        };
        let mut dispatcher = FakeDispatcher::default();
        let outcome = drive_goal(
            "goal-review",
            &grant(),
            GoalControlBudget::new(4, 16).unwrap(),
            &mut controller,
            &mut dispatcher,
        )
        .unwrap();

        assert_eq!(
            outcome.cycle_reports[0]["final_status"],
            Value::from("succeeded")
        );
        assert_eq!(outcome.final_status, GoalControlStatus::Stopped);
        assert_eq!(outcome.report["final_status"], Value::from("stopped"));
        assert_eq!(
            outcome.report["disposition"]["reason"],
            Value::from("needs_review")
        );
        verify_goal_control_report(&outcome.report).expect("stop report verifies");
    }

    #[test]
    fn aggregate_dispatch_ceiling_narrows_the_last_mission_and_prevents_an_extra_call() {
        let mut controller = |_: &GoalControlContext<'_>| {
            Ok(GoalDecision::RunMission(mission(
                "budgeted-private-mission",
            )))
        };
        let mut dispatcher = FakeDispatcher::default();
        let outcome = drive_goal(
            "goal-budget",
            &grant(),
            GoalControlBudget::new(5, 1).unwrap(),
            &mut controller,
            &mut dispatcher,
        )
        .unwrap();

        assert_eq!(dispatcher.calls, 1);
        assert_eq!(outcome.report["total_dispatches"], Value::from(1));
        assert_eq!(
            outcome.cycle_reports[0]["totals"]["max_attempts"],
            Value::from(1),
            "the cycle's embedded grant must be narrowed to the aggregate remainder"
        );
        assert_eq!(
            outcome.final_status,
            GoalControlStatus::DispatchBudgetExhausted
        );
        verify_goal_control_report(&outcome.report).expect("exhaustion report verifies");
    }

    #[test]
    fn ambiguous_dispatch_stops_the_goal_before_the_controller_can_retry_it() {
        let mut controller_calls = 0;
        let mut controller = |_: &GoalControlContext<'_>| {
            controller_calls += 1;
            Ok(GoalDecision::RunMission(mission(
                "ambiguous-private-mission",
            )))
        };
        let mut dispatcher = FakeDispatcher {
            calls: 0,
            fail: true,
        };
        let outcome = drive_goal(
            "goal-unknown",
            &grant(),
            GoalControlBudget::new(4, 16).unwrap(),
            &mut controller,
            &mut dispatcher,
        )
        .unwrap();

        assert_eq!(outcome.final_status, GoalControlStatus::OutcomeUnknown);
        assert_eq!(outcome.dispatches_this_call, 1);
        assert_eq!(dispatcher.calls, 1);
        assert_eq!(controller_calls, 1);
        assert_eq!(
            outcome.report["final_status"],
            Value::from("outcome_unknown")
        );
        assert!(!outcome
            .report
            .to_string()
            .contains("private transport detail"));
        verify_goal_control_report(&outcome.report).expect("unknown report verifies");
    }

    #[test]
    fn a_tampered_goal_cycle_digest_is_rejected_even_when_the_outer_report_is_unchanged() {
        let mut controller = |context: &GoalControlContext<'_>| {
            if context.completed_cycles() == 0 {
                Ok(GoalDecision::RunMission(mission("digest-private-mission")))
            } else {
                Ok(GoalDecision::Stop(GoalStopReason::NoAdmissibleWork))
            }
        };
        let mut dispatcher = FakeDispatcher::default();
        let outcome = drive_goal(
            "goal-digest",
            &grant(),
            GoalControlBudget::new(2, 8).unwrap(),
            &mut controller,
            &mut dispatcher,
        )
        .unwrap();
        let mut tampered = outcome.report;
        tampered["cycles"][0]["mission_sha256"] = Value::String("f".repeat(64));

        assert!(verify_goal_control_report(&tampered).is_err());
    }

    #[test]
    fn a_controller_error_preserves_prior_cycles_but_never_echoes_its_text() {
        let mut controller = |context: &GoalControlContext<'_>| {
            if context.completed_cycles() == 0 {
                Ok(GoalDecision::RunMission(mission(
                    "controller-private-mission",
                )))
            } else {
                Err("prompt-secret-and-provider-payload".into())
            }
        };
        let mut dispatcher = FakeDispatcher::default();
        let outcome = drive_goal(
            "goal-controller-error",
            &grant(),
            GoalControlBudget::new(2, 8).unwrap(),
            &mut controller,
            &mut dispatcher,
        )
        .unwrap();

        assert_eq!(outcome.final_status, GoalControlStatus::Refused);
        assert_eq!(outcome.report["cycles"].as_array().unwrap().len(), 1);
        assert!(!outcome.report.to_string().contains("prompt-secret"));
        assert!(outcome.report["disposition"]["error_sha256"].is_string());
        verify_goal_control_report(&outcome.report).expect("controller refusal verifies");
    }

    #[test]
    fn a_panicking_goal_controller_preserves_the_completed_cycle_without_echoing_panic_text() {
        let mut controller = |context: &GoalControlContext<'_>| {
            if context.completed_cycles() == 0 {
                Ok(GoalDecision::RunMission(mission("panic-preserved-mission")))
            } else {
                panic!("private evaluator panic payload");
            }
        };
        let mut dispatcher = FakeDispatcher::default();
        let outcome = drive_goal(
            "goal-controller-panic",
            &grant(),
            GoalControlBudget::new(2, 8).unwrap(),
            &mut controller,
            &mut dispatcher,
        )
        .expect("a caught controller panic becomes a reportable refusal");

        assert_eq!(outcome.final_status, GoalControlStatus::Refused);
        assert_eq!(dispatcher.calls, 1);
        assert_eq!(outcome.cycle_reports.len(), 1);
        assert_eq!(outcome.report["cycles"].as_array().unwrap().len(), 1);
        assert_eq!(outcome.report["cycles"][0]["final_status"], "succeeded");
        assert_eq!(outcome.report["disposition"]["kind"], "controller_error");
        assert!(outcome.report["disposition"]["error_sha256"].is_string());
        assert!(!outcome
            .report
            .to_string()
            .contains("private evaluator panic payload"));
        verify_goal_control_report(&outcome.report).expect("the preserved refusal report verifies");
    }
}
