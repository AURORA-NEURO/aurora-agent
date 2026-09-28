//! MCP adapter for the grant-gated autonomous mission driver.
//!
//! The planner and drive kernel stay transport-agnostic in `bioprism-autopilot`. This adapter
//! keeps the MCP boundary explicit: previews are the default, every executing request carries a
//! validated autonomy grant, and reports can be independently checked after they are retained by
//! the caller.

use bioprism_autopilot::drive::instantiation_mission;
use bioprism_autopilot::{
    AttemptKind, AttemptRecord, AutonomyGrant, AutopilotError, BoundedDriveOptions, DriveHistory,
    FinalStatus, GoalControlBudget, GoalControlStatus, GoalDecision, GoalStopReason, NextAction,
    drive_goal, drive_instantiation_bounded, preview_first_action, resume_goal_from_checkpoint,
    resume_instantiation_bounded, seal_autopilot_checkpoint, seal_goal_control_checkpoint,
    validate_goal_control_checkpoint, verify_autopilot_report, verify_goal_control_report,
};
use bioprism_ids::ContentHash;
use serde::Deserialize;
use serde_json::{Value, json};
use std::sync::atomic::AtomicBool;

use super::Server;

const MAX_AUTOPILOT_REQUEST_BYTES: usize = 20_000_000;
const MAX_AUTOPILOT_REHYDRATED_ATTEMPTS: usize = 16;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct AutopilotDriveRequest {
    instantiation: Value,
    grant: AutonomyGrant,
    #[serde(default)]
    mode: AutopilotDriveMode,
    #[serde(default)]
    max_dispatches_this_call: Option<usize>,
    #[serde(default)]
    checkpoint: Option<Value>,
    #[serde(default)]
    rehydrated_attempts: Option<Vec<Value>>,
}

#[derive(Debug, Clone, Copy, Default, Deserialize)]
#[serde(rename_all = "snake_case")]
enum AutopilotDriveMode {
    #[default]
    Preview,
    Execute,
    Resume,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct AutopilotVerifyRequest {
    report: Value,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct AutopilotGoalStepRequest {
    goal_id: String,
    grant: AutonomyGrant,
    #[serde(default)]
    budget: Option<AutopilotGoalBudgetRequest>,
    decision: Value,
    #[serde(default)]
    checkpoint: Option<Value>,
    #[serde(default)]
    previous_autopilot_report: Option<Value>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct AutopilotGoalBudgetRequest {
    max_cycles: usize,
    max_total_dispatches: usize,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct AutopilotGoalVerifyRequest {
    report: Value,
    #[serde(default)]
    checkpoint: Option<Value>,
}

fn parse_goal_decision(value: &Value) -> Result<GoalDecision, String> {
    let kind = value
        .get("kind")
        .and_then(Value::as_str)
        .ok_or_else(|| "goal decision kind must be a string".to_string())?;
    match kind {
        "run_mission" => {
            exact_fields(value, &["kind", "mission"], "run_mission decision")?;
            if !value["mission"].is_object() {
                return Err("goal decision mission must be an object".into());
            }
            Ok(GoalDecision::RunMission(value["mission"].clone()))
        }
        "complete" => {
            exact_fields(
                value,
                &["kind", "evaluator_id", "evidence_sha256"],
                "complete decision",
            )?;
            Ok(GoalDecision::Complete {
                evaluator_id: value["evaluator_id"]
                    .as_str()
                    .ok_or_else(|| "evaluator_id must be a string".to_string())?
                    .to_string(),
                evidence_sha256: value["evidence_sha256"]
                    .as_str()
                    .ok_or_else(|| "evidence_sha256 must be a string".to_string())?
                    .to_string(),
            })
        }
        "stop" => {
            exact_fields(value, &["kind", "reason"], "stop decision")?;
            let reason = match value["reason"].as_str() {
                Some("no_admissible_work") => GoalStopReason::NoAdmissibleWork,
                Some("needs_review") => GoalStopReason::NeedsReview,
                Some("cancelled") => GoalStopReason::Cancelled,
                Some("abandoned") => GoalStopReason::Abandoned,
                _ => return Err("stop reason is not a supported goal stop state".into()),
            };
            Ok(GoalDecision::Stop(reason))
        }
        _ => Err("goal decision kind must be run_mission, complete, or stop".into()),
    }
}

fn check_request_size(arguments: &Value, operation: &str) -> Result<(), String> {
    let encoded = serde_json::to_vec(arguments)
        .map_err(|error| format!("cannot encode {operation} input: {error}"))?;
    if encoded.len() > MAX_AUTOPILOT_REQUEST_BYTES {
        return Err(format!(
            "{operation} input exceeds the {MAX_AUTOPILOT_REQUEST_BYTES}-byte safety bound"
        ));
    }
    Ok(())
}

fn drive_report_dispatch_metadata(
    report: &Value,
    attempts_before_call: u64,
    dispatches_this_call: u64,
) -> Result<DriveReportDispatchMetadata, String> {
    let report_attempts_used = report
        .pointer("/totals/attempts_used")
        .and_then(Value::as_u64)
        .ok_or_else(|| "generated autopilot report is missing totals.attempts_used".to_string())?;
    let attempts_used = attempts_before_call
        .checked_add(dispatches_this_call)
        .ok_or_else(|| {
            "observed autopilot dispatch count overflows total attempts_used".to_string()
        })?;
    let report_sha256 = report
        .get("report_sha256")
        .and_then(Value::as_str)
        .ok_or_else(|| "generated autopilot report is missing report_sha256".to_string())?
        .to_string();
    Ok(DriveReportDispatchMetadata {
        attempts_used,
        attempts_this_call: dispatches_this_call,
        report_attempts_used,
        report_accounting_matches: report_attempts_used == attempts_used,
        report_sha256,
    })
}

#[derive(Debug)]
struct DriveReportDispatchMetadata {
    attempts_used: u64,
    attempts_this_call: u64,
    report_attempts_used: u64,
    report_accounting_matches: bool,
    report_sha256: String,
}

fn exact_fields(value: &Value, expected: &[&str], label: &str) -> Result<(), String> {
    let object = value
        .as_object()
        .ok_or_else(|| format!("{label} must be an object"))?;
    if object.len() != expected.len() || expected.iter().any(|key| !object.contains_key(*key)) {
        return Err(format!("{label} has missing or unsupported fields"));
    }
    Ok(())
}

fn attempt_kind(value: &Value) -> Result<AttemptKind, String> {
    match value.as_str() {
        Some("full") => Ok(AttemptKind::Full),
        Some("repair") => Ok(AttemptKind::Repair),
        _ => Err("rehydrated attempt kind must be full or repair".into()),
    }
}

fn decode_rehydrated_attempts(values: Vec<Value>) -> Result<Vec<AttemptRecord>, String> {
    if values.len() > MAX_AUTOPILOT_REHYDRATED_ATTEMPTS {
        return Err(format!(
            "rehydrated attempt history exceeds the {MAX_AUTOPILOT_REHYDRATED_ATTEMPTS}-attempt bound"
        ));
    }
    values
        .into_iter()
        .map(|row| {
            let kind = attempt_kind(
                row.get("kind")
                    .ok_or_else(|| "rehydrated attempt kind is missing".to_string())?,
            )?;
            let mission = row
                .get("mission")
                .cloned()
                .ok_or_else(|| "rehydrated attempt mission is missing".to_string())?;
            match row.get("delivery").and_then(Value::as_str) {
                Some("delivered") => {
                    exact_fields(
                        &row,
                        &[
                            "kind",
                            "mission",
                            "delivery",
                            "report",
                            "reconciliation",
                            "reconciliation_note",
                        ],
                        "delivered rehydrated attempt",
                    )?;
                    let note = match row.get("reconciliation_note") {
                        Some(Value::Null) => None,
                        Some(Value::String(note))
                            if note.len() <= 16_384 && !note.contains('\0') =>
                        {
                            Some(note.clone())
                        }
                        _ => return Err("reconciliation_note must be null or bounded text".into()),
                    };
                    let reconciliation = match row.get("reconciliation") {
                        Some(Value::Null) => None,
                        Some(value) => Some(value.clone()),
                        None => return Err("reconciliation field is missing".into()),
                    };
                    AttemptRecord::delivered(
                        kind,
                        mission,
                        row["report"].clone(),
                        reconciliation,
                        note,
                    )
                    .map_err(|error| format!("invalid delivered rehydrated attempt: {error}"))
                }
                Some("undelivered") => {
                    exact_fields(
                        &row,
                        &["kind", "mission", "delivery", "dispatch_error"],
                        "undelivered rehydrated attempt",
                    )?;
                    let error = row["dispatch_error"]
                        .as_str()
                        .filter(|error| {
                            !error.is_empty() && error.len() <= 16_384 && !error.contains('\0')
                        })
                        .ok_or_else(|| {
                            "dispatch_error must be bounded non-empty text".to_string()
                        })?;
                    AttemptRecord::undelivered(kind, mission, error.to_owned())
                        .map_err(|error| format!("invalid undelivered rehydrated attempt: {error}"))
                }
                _ => Err("rehydrated attempt delivery must be delivered or undelivered".into()),
            }
        })
        .collect()
}

fn encode_rehydrated_attempts(history: &DriveHistory) -> Vec<Value> {
    history
        .attempts()
        .iter()
        .map(|attempt| {
            if let Some(report) = attempt.report() {
                json!({
                    "kind": attempt.kind().as_str(),
                    "mission": attempt.mission(),
                    "delivery": "delivered",
                    "report": report,
                    "reconciliation": attempt.reconciliation(),
                    "reconciliation_note": attempt.reconciliation_note(),
                })
            } else {
                json!({
                    "kind": attempt.kind().as_str(),
                    "mission": attempt.mission(),
                    "delivery": "undelivered",
                    "dispatch_error": attempt.dispatch_error().unwrap_or_default(),
                })
            }
        })
        .collect()
}

fn sealed_checkpoint_callback<'a>(
    grant: &'a AutonomyGrant,
    latest: &'a mut Option<Value>,
    previous_digest: &'a mut Option<String>,
) -> impl FnMut(&DriveHistory) -> Result<(), AutopilotError> + 'a {
    move |history| {
        let snapshot = seal_autopilot_checkpoint(
            grant,
            history,
            history.dispatches_used() as u64,
            previous_digest.as_deref(),
        )?;
        *previous_digest = snapshot
            .get("snapshot_digest")
            .and_then(Value::as_str)
            .map(str::to_owned);
        *latest = Some(snapshot);
        Ok(())
    }
}

fn continuation_package(
    request: &AutopilotDriveRequest,
    checkpoint: &Value,
    rehydrated_attempts: &[Value],
    max_dispatches_this_call: usize,
) -> Value {
    let next_request = json!({
        "instantiation": request.instantiation.clone(),
        "grant": request.grant.clone(),
        "mode": "resume",
        "max_dispatches_this_call": max_dispatches_this_call,
        "checkpoint": checkpoint.clone(),
        "rehydrated_attempts": rehydrated_attempts,
    });
    let fits = serde_json::to_vec(&next_request)
        .is_ok_and(|encoded| encoded.len() <= MAX_AUTOPILOT_REQUEST_BYTES);
    if fits {
        json!({
            "available": true,
            "next_request": next_request,
            "private_material": "mission arguments, reports, reconciliation records, and dispatch errors; retain as private operational data",
        })
    } else {
        json!({
            "available": false,
            "checkpoint": checkpoint,
            "reason": "the complete rehydration package exceeds the MCP 20 MB request bound; the drive is paused but cannot continue through this endpoint, so stop rather than replay",
            "private_material": "mission arguments, reports, reconciliation records, and dispatch errors; retain as private operational data",
        })
    }
}

impl Server {
    /// Preview or execute one autonomy-grant-bounded drive over a retained workflow instantiation.
    pub(super) fn autopilot_drive(&self, arguments: &Value) -> Result<Value, String> {
        check_request_size(arguments, "autopilot drive")?;
        let request: AutopilotDriveRequest = serde_json::from_value(arguments.clone())
            .map_err(|error| format!("invalid autopilot drive input: {error}"))?;
        let grant_digest = request
            .grant
            .digest()
            .map_err(|error| format!("cannot digest autonomy grant: {error}"))?;

        match request.mode {
            AutopilotDriveMode::Preview => {
                if request.max_dispatches_this_call.is_some()
                    || request.checkpoint.is_some()
                    || request.rehydrated_attempts.is_some()
                {
                    return Err(
                        "autopilot preview does not accept dispatch limits or recovery state"
                            .into(),
                    );
                }
                let mission = instantiation_mission(&request.instantiation)
                    .map_err(|error| format!("autopilot preview refused: {error}"))?;
                let action = preview_first_action(&request.grant, mission)
                    .map_err(|error| format!("autopilot preview refused: {error}"))?;
                let (mission, authorization) = match action {
                    NextAction::DispatchFull {
                        mission,
                        authorization,
                    } => (mission, authorization),
                    other => {
                        return Err(format!(
                            "autopilot planner returned {other:?} for an empty drive instead of a full dispatch"
                        ));
                    }
                };
                let mission_digest = ContentHash::of_value(&mission)
                    .map_err(|error| format!("cannot digest planned autopilot mission: {error}"))?
                    .to_string();
                let step_count = mission
                    .get("steps")
                    .and_then(Value::as_array)
                    .map_or(0, Vec::len);
                let mission_id = mission
                    .get("mission_id")
                    .and_then(Value::as_str)
                    .unwrap_or("unknown");
                Ok(json!({
                    "ok": true,
                    "workflow": "autopilot_drive",
                    "mode": "preview",
                    "dry_run": true,
                    "no_dispatch": true,
                    "dispatch": "not_started",
                    "execution": "not_started",
                    "writes": "none",
                    "grant_digest": grant_digest,
                    "max_attempts": request.grant.max_attempts(),
                    "planned_first_action": {
                        "action": "dispatch_full",
                        "attempt_index": authorization.attempt_index(),
                        "mission_id": mission_id,
                        "step_count": step_count,
                        "planned_mission_digest": mission_digest,
                        "allowed_tools": request.grant.allowed_tools(),
                        "allow_side_effects": request.grant.allow_side_effects(),
                        "mission": mission,
                    },
                }))
            }
            AutopilotDriveMode::Execute => {
                if request.checkpoint.is_some() || request.rehydrated_attempts.is_some() {
                    return Err(
                        "autopilot execute starts a new drive and cannot accept recovery state; use mode resume"
                            .into(),
                    );
                }
                let max_dispatches_this_call = request
                    .max_dispatches_this_call
                    .unwrap_or_else(|| request.grant.max_attempts());
                let cancellation = AtomicBool::new(false);
                let mut dispatches_this_call = 0_u64;
                let mut dispatch = |mission: &Value| {
                    dispatches_this_call = dispatches_this_call
                        .checked_add(1)
                        .ok_or_else(|| "MCP dispatch count overflowed".to_string())?;
                    self.execute_agent_mission_with_cancellation(mission, &cancellation)
                };
                let mut logical_wait_ticks = Vec::new();
                let mut waiter = |delay_ticks| {
                    logical_wait_ticks.push(delay_ticks);
                    Ok(())
                };
                let mut latest_checkpoint = None;
                let mut previous_digest = None;
                let mut rehydrated_attempts = Vec::new();
                let mut seal_checkpoint = sealed_checkpoint_callback(
                    &request.grant,
                    &mut latest_checkpoint,
                    &mut previous_digest,
                );
                let mut checkpoint = |history: &DriveHistory| {
                    seal_checkpoint(history)?;
                    rehydrated_attempts = encode_rehydrated_attempts(history);
                    Ok(())
                };
                let outcome = drive_instantiation_bounded(
                    &request.grant,
                    &request.instantiation,
                    &mut dispatch,
                    BoundedDriveOptions::new(max_dispatches_this_call, &mut waiter),
                    &mut checkpoint,
                )
                .map_err(|error| format!("autopilot drive refused: {error}"))?;
                drop(checkpoint);
                drop(seal_checkpoint);
                drop(dispatch);
                let report = outcome.report;
                let verification = verify_autopilot_report(&report).map_err(|error| {
                    format!("cannot verify generated autopilot report: {error}")
                })?;
                let dispatch_metadata =
                    drive_report_dispatch_metadata(&report, 0, dispatches_this_call)?;
                let succeeded = outcome.final_status == FinalStatus::Succeeded
                    && verification.get("valid") == Some(&Value::Bool(true))
                    && dispatch_metadata.report_accounting_matches;
                let paused = outcome.final_status == FinalStatus::Paused;
                let recovery = if paused {
                    match latest_checkpoint.as_ref() {
                        Some(checkpoint) => continuation_package(
                            &request,
                            checkpoint,
                            &rehydrated_attempts,
                            max_dispatches_this_call,
                        ),
                        None => json!({
                            "available": false,
                            "reason": "the paused drive has no sealed checkpoint; stop rather than replay"
                        }),
                    }
                } else {
                    Value::Null
                };

                Ok(json!({
                    "ok": succeeded,
                    "workflow": "autopilot_drive",
                    "mode": "execute",
                    "dry_run": false,
                    "paused": paused,
                    "dispatch_started": dispatch_metadata.attempts_this_call > 0,
                    "attempts_this_call": dispatch_metadata.attempts_this_call,
                    "attempts_used": dispatch_metadata.attempts_used,
                    "report_attempts_used": dispatch_metadata.report_attempts_used,
                    "attempt_accounting_matches": dispatch_metadata.report_accounting_matches,
                    "max_dispatches_this_call": max_dispatches_this_call,
                    "max_attempts": request.grant.max_attempts(),
                    "grant_digest": grant_digest,
                    "report_sha256": dispatch_metadata.report_sha256,
                    "report_verification": verification,
                    "scheduling": {
                        "logical_wait_ticks": logical_wait_ticks,
                        "wall_clock_waited": false
                    },
                    "recovery": recovery,
                    "report": report,
                }))
            }
            AutopilotDriveMode::Resume => {
                let checkpoint_snapshot = request
                    .checkpoint
                    .as_ref()
                    .ok_or_else(|| "autopilot resume requires checkpoint".to_string())?;
                let attempts = request
                    .rehydrated_attempts
                    .clone()
                    .ok_or_else(|| "autopilot resume requires rehydrated_attempts".to_string())?;
                let attempts = decode_rehydrated_attempts(attempts)?;
                let max_dispatches_this_call = request
                    .max_dispatches_this_call
                    .unwrap_or_else(|| request.grant.max_attempts());
                let mut previous_digest = checkpoint_snapshot
                    .get("snapshot_digest")
                    .and_then(Value::as_str)
                    .map(str::to_owned);
                let mut latest_checkpoint = None;
                let mut rehydrated_attempts = Vec::new();
                let mut seal_checkpoint = sealed_checkpoint_callback(
                    &request.grant,
                    &mut latest_checkpoint,
                    &mut previous_digest,
                );
                let mut checkpoint = |history: &DriveHistory| {
                    seal_checkpoint(history)?;
                    rehydrated_attempts = encode_rehydrated_attempts(history);
                    Ok(())
                };
                let cancellation = AtomicBool::new(false);
                let mut dispatches_this_call = 0_u64;
                let mut dispatch = |mission: &Value| {
                    dispatches_this_call = dispatches_this_call
                        .checked_add(1)
                        .ok_or_else(|| "MCP dispatch count overflowed".to_string())?;
                    self.execute_agent_mission_with_cancellation(mission, &cancellation)
                };
                let mut logical_wait_ticks = Vec::new();
                let mut waiter = |delay_ticks| {
                    logical_wait_ticks.push(delay_ticks);
                    Ok(())
                };
                let outcome = resume_instantiation_bounded(
                    &request.grant,
                    checkpoint_snapshot,
                    &request.instantiation,
                    attempts,
                    &mut dispatch,
                    BoundedDriveOptions::new(max_dispatches_this_call, &mut waiter),
                    &mut checkpoint,
                )
                .map_err(|error| format!("autopilot resume refused: {error}"))?;
                drop(checkpoint);
                drop(seal_checkpoint);
                drop(dispatch);
                let report = outcome.report;
                let verification = verify_autopilot_report(&report)
                    .map_err(|error| format!("cannot verify resumed autopilot report: {error}"))?;
                let attempts_before_call = checkpoint_snapshot
                    .get("attempts_used")
                    .and_then(Value::as_u64)
                    .ok_or_else(|| {
                        "validated autopilot checkpoint is missing attempts_used".to_string()
                    })?;
                let dispatch_metadata = drive_report_dispatch_metadata(
                    &report,
                    attempts_before_call,
                    dispatches_this_call,
                )?;
                let succeeded = outcome.final_status == FinalStatus::Succeeded
                    && verification.get("valid") == Some(&Value::Bool(true))
                    && dispatch_metadata.report_accounting_matches;
                let paused = outcome.final_status == FinalStatus::Paused;
                let recovery = if paused {
                    match latest_checkpoint.as_ref() {
                        Some(checkpoint) => continuation_package(
                            &request,
                            checkpoint,
                            &rehydrated_attempts,
                            max_dispatches_this_call,
                        ),
                        None => json!({
                            "available": false,
                            "reason": "the paused drive has no sealed checkpoint; stop rather than replay"
                        }),
                    }
                } else {
                    Value::Null
                };
                Ok(json!({
                    "ok": succeeded,
                    "workflow": "autopilot_drive",
                    "mode": "resume",
                    "dry_run": false,
                    "paused": paused,
                    "dispatch_started": dispatch_metadata.attempts_this_call > 0,
                    "attempts_this_call": dispatch_metadata.attempts_this_call,
                    "attempts_used": dispatch_metadata.attempts_used,
                    "report_attempts_used": dispatch_metadata.report_attempts_used,
                    "attempt_accounting_matches": dispatch_metadata.report_accounting_matches,
                    "max_dispatches_this_call": max_dispatches_this_call,
                    "max_attempts": request.grant.max_attempts(),
                    "grant_digest": grant_digest,
                    "report_sha256": dispatch_metadata.report_sha256,
                    "report_verification": verification,
                    "scheduling": {
                        "logical_wait_ticks": logical_wait_ticks,
                        "wall_clock_waited": false
                    },
                    "recovery": recovery,
                    "report": report,
                }))
            }
        }
    }

    /// Recompute the seal and required limitation set on a caller-retained autopilot report.
    pub(super) fn autopilot_verify(&self, arguments: &Value) -> Result<Value, String> {
        check_request_size(arguments, "autopilot verification")?;
        let request: AutopilotVerifyRequest = serde_json::from_value(arguments.clone())
            .map_err(|error| format!("invalid autopilot verification input: {error}"))?;
        let verification = verify_autopilot_report(&request.report)
            .map_err(|error| format!("autopilot report cannot be verified: {error}"))?;
        let valid = verification.get("valid") == Some(&Value::Bool(true));
        let report_sha256 = request
            .report
            .get("report_sha256")
            .cloned()
            .unwrap_or(Value::Null);
        Ok(json!({
            "ok": valid,
            "workflow": "autopilot_verify",
            "report_sha256": report_sha256,
            "verification": verification,
            "dispatch": "not_started",
            "writes": "none",
        }))
    }

    /// Execute exactly one caller-supplied goal transition and return resumable metadata on a
    /// safe stop. Planning, evaluation, and private checkpoint storage remain caller-owned.
    pub(super) fn autopilot_goal_step(&self, arguments: &Value) -> Result<Value, String> {
        check_request_size(arguments, "autopilot goal step")?;
        let request: AutopilotGoalStepRequest = serde_json::from_value(arguments.clone())
            .map_err(|error| format!("invalid autopilot goal step input: {error}"))?;
        let decision = parse_goal_decision(&request.decision)?;

        let (budget, checkpoint, generation, previous_snapshot_digest) =
            if let Some(checkpoint) = request.checkpoint.as_ref() {
                if request.budget.is_some() {
                    return Err(
                        "goal resume reuses the checkpoint budget; do not supply budget".into(),
                    );
                }
                let checkpoint = validate_goal_control_checkpoint(checkpoint)
                    .map_err(|error| format!("goal checkpoint is invalid: {error}"))?;
                if checkpoint["goal_control_report"]["goal_id"] != request.goal_id {
                    return Err("goal_id does not match the checkpoint".into());
                }
                let current_generation = checkpoint["generation"]
                    .as_u64()
                    .ok_or_else(|| "goal checkpoint generation is missing".to_string())?;
                let generation = current_generation
                    .checked_add(1)
                    .ok_or_else(|| "goal checkpoint generation overflowed".to_string())?;
                let previous = checkpoint["snapshot_digest"]
                    .as_str()
                    .ok_or_else(|| "goal checkpoint digest is missing".to_string())?
                    .to_string();
                (None, Some(checkpoint), generation, Some(previous))
            } else {
                if request.previous_autopilot_report.is_some() {
                    return Err("previous_autopilot_report requires a goal checkpoint".into());
                }
                let budget = request
                    .budget
                    .as_ref()
                    .ok_or_else(|| "a new goal requires an explicit budget".to_string())?;
                let budget = GoalControlBudget::new(budget.max_cycles, budget.max_total_dispatches)
                    .map_err(|error| format!("invalid goal budget: {error}"))?;
                (Some(budget), None, 1, None)
            };

        let mut pending_decision = Some(decision);
        let mut controller = move |_context: &bioprism_autopilot::GoalControlContext<'_>| {
            Ok(pending_decision
                .take()
                .unwrap_or(GoalDecision::Stop(GoalStopReason::NoAdmissibleWork)))
        };
        let cancellation = AtomicBool::new(false);
        let mut dispatcher =
            |mission: &Value| self.execute_agent_mission_with_cancellation(mission, &cancellation);
        let outcome = match checkpoint.as_ref() {
            Some(checkpoint) => resume_goal_from_checkpoint(
                checkpoint,
                request.previous_autopilot_report,
                &request.grant,
                &mut controller,
                &mut dispatcher,
            )
            .map_err(|error| format!("autopilot goal resume refused: {error}"))?,
            None => drive_goal(
                &request.goal_id,
                &request.grant,
                budget.ok_or_else(|| "a new goal requires an explicit budget".to_string())?,
                &mut controller,
                &mut dispatcher,
            )
            .map_err(|error| format!("autopilot goal step refused: {error}"))?,
        };
        let report_verification = verify_goal_control_report(&outcome.report)
            .map_err(|error| format!("cannot verify generated goal report: {error}"))?;
        let goal_status = outcome.final_status.as_str();
        let safe_stop = outcome.final_status == GoalControlStatus::Stopped
            && matches!(
                outcome
                    .report
                    .pointer("/disposition/reason")
                    .and_then(Value::as_str),
                Some("no_admissible_work" | "needs_review")
            );
        let goal_checkpoint = if safe_stop {
            Some(
                seal_goal_control_checkpoint(
                    &outcome.report,
                    generation,
                    previous_snapshot_digest.as_deref(),
                )
                .map_err(|error| format!("cannot seal safe goal checkpoint: {error}"))?,
            )
        } else {
            None
        };
        let dispatch_started = outcome.dispatches_this_call > 0;
        let goal_ok = !matches!(
            outcome.final_status,
            GoalControlStatus::OutcomeUnknown | GoalControlStatus::Refused
        );
        let report_sha256 = outcome.report["report_sha256"].clone();
        let checkpoint_generation = goal_checkpoint
            .as_ref()
            .map(|checkpoint| checkpoint["generation"].clone())
            .unwrap_or(Value::Null);

        Ok(json!({
            "ok": goal_ok && report_verification["valid"] == true,
            "workflow": "autopilot_goal_step",
            "goal_id": request.goal_id,
            "goal_status": goal_status,
            "goal_complete": outcome.final_status == GoalControlStatus::Completed,
            "dispatch_started": dispatch_started,
            "dispatches_this_call": outcome.dispatches_this_call,
            "total_dispatches": outcome.report["total_dispatches"],
            "report_sha256": report_sha256,
            "report_verification": report_verification,
            "report": outcome.report,
            "cycle_autopilot_reports": outcome.cycle_reports,
            "checkpoint_generation": checkpoint_generation,
            "checkpoint": goal_checkpoint,
            "checkpoint_storage": "caller_owned",
            "writes": "none",
        }))
    }

    /// Independently verify one goal-control report and, when supplied, its checkpoint binding.
    pub(super) fn autopilot_goal_verify(&self, arguments: &Value) -> Result<Value, String> {
        check_request_size(arguments, "autopilot goal verification")?;
        let request: AutopilotGoalVerifyRequest = serde_json::from_value(arguments.clone())
            .map_err(|error| format!("invalid autopilot goal verification input: {error}"))?;
        let report_verification = verify_goal_control_report(&request.report)
            .map_err(|error| format!("goal-control report cannot be verified: {error}"))?;
        let checkpoint_verification = request
            .checkpoint
            .as_ref()
            .map(|checkpoint| -> Result<Value, String> {
                let normalized = validate_goal_control_checkpoint(checkpoint)
                    .map_err(|error| format!("goal checkpoint cannot be verified: {error}"))?;
                if normalized["goal_control_report"] != request.report {
                    return Err("goal checkpoint does not bind the supplied report".into());
                }
                Ok(json!({
                    "valid": true,
                    "generation": normalized["generation"],
                    "snapshot_digest": normalized["snapshot_digest"],
                }))
            })
            .transpose()?;
        let valid = report_verification["valid"] == true
            && checkpoint_verification
                .as_ref()
                .is_none_or(|verification| verification["valid"] == true);
        Ok(json!({
            "ok": valid,
            "workflow": "autopilot_goal_verify",
            "report_verification": report_verification,
            "checkpoint_verification": checkpoint_verification,
            "dispatch": "not_started",
            "writes": "none",
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::drive_report_dispatch_metadata;
    use serde_json::json;

    #[test]
    fn dispatch_metadata_reports_only_counters_present_in_the_report() {
        let report = json!({
            "totals": { "attempts_used": 4 },
            "report_sha256": "a".repeat(64),
        });

        assert_eq!(
            {
                let metadata = drive_report_dispatch_metadata(&report, 2, 2).unwrap();
                (
                    metadata.attempts_used,
                    metadata.attempts_this_call,
                    metadata.report_attempts_used,
                    metadata.report_accounting_matches,
                    metadata.report_sha256,
                )
            },
            (4, 2, 4, true, "a".repeat(64))
        );
    }

    #[test]
    fn dispatch_metadata_preserves_observed_attempts_when_report_accounting_regresses() {
        let regressed_count = json!({
            "totals": { "attempts_used": 1 },
            "report_sha256": "a".repeat(64),
        });
        let metadata = drive_report_dispatch_metadata(&regressed_count, 2, 1).unwrap();
        assert_eq!(metadata.attempts_used, 3);
        assert_eq!(metadata.attempts_this_call, 1);
        assert_eq!(metadata.report_attempts_used, 1);
        assert!(!metadata.report_accounting_matches);

        let no_dispatch = json!({
            "totals": { "attempts_used": 2 },
            "report_sha256": "a".repeat(64),
        });
        let metadata = drive_report_dispatch_metadata(&no_dispatch, 2, 0).unwrap();
        assert_eq!(metadata.attempts_used, 2);
        assert_eq!(metadata.attempts_this_call, 0);
        assert!(metadata.report_accounting_matches);
    }

    #[test]
    fn dispatch_metadata_refuses_missing_evidence_instead_of_defaulting() {
        let missing_count = json!({ "report_sha256": "a".repeat(64) });
        assert!(
            drive_report_dispatch_metadata(&missing_count, 0, 0)
                .unwrap_err()
                .contains("missing totals.attempts_used")
        );

        let missing_digest = json!({ "totals": { "attempts_used": 0 } });
        assert!(
            drive_report_dispatch_metadata(&missing_digest, 0, 0)
                .unwrap_err()
                .contains("missing report_sha256")
        );
    }
}
