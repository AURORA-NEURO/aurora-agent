//! The effectful shell: obey the planner, dispatch through a caller-supplied seam, collect
//! receipts.
//!
//! The seam is [`MissionDispatch`], one function from a mission document to a mission report.
//! The CLI supplies a closure over the in-process MCP server's execute-mission boundary — the
//! same boundary the transport uses — and tests supply fakes. This crate never links the MCP
//! server: keeping the dispatcher on the caller's side of the boundary is what lets the whole
//! kernel stay pure and lets a reviewer read the drive loop as "call [`plan_next_action`],
//! record what came back, repeat".
//!
//! # Where reconciliation comes from
//!
//! When the dispatched mission carries a `workflow_binding` — every instantiated mission does —
//! the mission boundary attaches a reconciliation to its own report, and the drive lifts it out
//! verbatim. When it does not, and the caller supplied the instantiation artifact, the drive
//! calls `reconcile_domain_workflow` for full dispatches; the grant's policy overwrite usually
//! makes the report's plan digest differ from the instantiation's, and that mismatch is then an
//! integrity finding the success rule refuses — recorded, not repaired. Absence is always
//! explained in the attempt's `reconciliation_note`; "not attempted" and "attempted and
//! refused" never share a representation.
//!
//! When neither source exists — no workflow binding on the mission and no instantiation artifact
//! supplied — and the grant requires a complete reconciliation, the drive refuses before its
//! first dispatch: the success rule is already provably unreachable, so the same refusal the
//! planner applies to a binding-less repair applies to attempt 1, before any side effect runs.

use crate::error::AutopilotError;
use crate::grant::AutonomyGrant;
use crate::history::{AttemptKind, AttemptRecord, DriveHistory};
use crate::planner::{never_dispatched_rows, plan_next_action, NextAction};
use crate::report::{build_autopilot_report, FinalDisposition, FinalStatus};
use bioprism_devplat::reconcile_domain_workflow;
use serde_json::{json, Value};

/// The one effect this crate performs, supplied by the caller.
///
/// `dispatch` receives the exact mission document the planner constructed and must return the
/// mission report JSON, or an error string when no report exists at all. Implemented for any
/// `FnMut(&Value) -> Result<Value, String>`.
pub trait MissionDispatch {
    fn dispatch(&mut self, mission: &Value) -> Result<Value, String>;
}

impl<F> MissionDispatch for F
where
    F: FnMut(&Value) -> Result<Value, String>,
{
    fn dispatch(&mut self, mission: &Value) -> Result<Value, String> {
        self(mission)
    }
}

/// A finished drive: the chained report and its typed final status.
#[derive(Debug, Clone)]
pub struct DriveOutcome {
    pub final_status: FinalStatus,
    pub report: Value,
}

fn lift_auto_attached_reconciliation(report: &Value) -> Option<Value> {
    let attached = report.get("workflow_reconciliation")?;
    if attached.get("present") == Some(&Value::Bool(true)) {
        Some(attached.clone())
    } else {
        None
    }
}

fn reconciliation_for_attempt(
    kind: AttemptKind,
    mission: &Value,
    report: &Value,
    instantiation: Option<&Value>,
) -> (Option<Value>, Option<String>) {
    if let Some(attached) = lift_auto_attached_reconciliation(report) {
        return (Some(attached), None);
    }
    if let Some(refused) = report.get("workflow_reconciliation") {
        let reason = refused
            .get("error")
            .and_then(Value::as_str)
            .unwrap_or("the mission boundary recorded a reconciliation failure");
        return (
            None,
            Some(format!("auto-attached reconciliation failed closed: {reason}")),
        );
    }
    if mission.get("workflow_binding").is_some_and(|binding| !binding.is_null()) {
        return (
            None,
            Some(
                "the dispatched mission carries a workflow binding but its report has no \
                 reconciliation; the dispatcher did not run the reconciling boundary"
                    .into(),
            ),
        );
    }
    match (kind, instantiation) {
        (AttemptKind::Full, Some(instantiation)) => {
            let request = json!({
                "instantiation": instantiation,
                "mission_report": report,
            });
            match reconcile_domain_workflow(&request) {
                Ok(record) => (Some(record), None),
                Err(error) => (
                    None,
                    Some(format!("reconcile_domain_workflow refused: {error}")),
                ),
            }
        }
        (AttemptKind::Full, None) => (
            None,
            Some(
                "no workflow binding and no instantiation artifact were supplied, so no \
                 reconciliation exists for this attempt"
                    .into(),
            ),
        ),
        (AttemptKind::Repair, _) => (
            None,
            Some(
                "a repair without a workflow binding has no honestly scoped reconciliation \
                 source"
                    .into(),
            ),
        ),
    }
}

/// Whether any reconciliation source can exist for a full dispatch of this history's mission.
/// The mission's own workflow binding is one source; the caller-supplied instantiation artifact
/// is the other. The planner cannot see the second — only the drive holds it — so this preflight
/// lives here.
fn reconciliation_source_exists(history: &DriveHistory, instantiation: Option<&Value>) -> bool {
    history.parsed_base().workflow_binding.is_some() || instantiation.is_some()
}

/// The accounting for a drive refused before its first dispatch: zero attempts used, every step
/// stated as never dispatched, and the reason the success rule was already unreachable.
fn reconciliation_unavailable_accounting(grant: &AutonomyGrant, history: &DriveHistory) -> Value {
    let step_ids = history
        .parsed_base()
        .steps
        .iter()
        .map(|step| step.id.clone())
        .collect::<Vec<_>>();
    json!({
        "reason": "reconciliation_unavailable",
        "detail": "the grant requires a complete reconciliation, but the mission carries no \
                   workflow binding and no instantiation artifact was supplied, so no attempt \
                   could ever carry one; the drive refuses before spending any dispatch rather \
                   than run side effects on an attempt that cannot reach success",
        "attempts_used": 0,
        "max_attempts": grant.max_attempts(),
        "unresolved_steps": never_dispatched_rows(&step_ids),
    })
}

fn drive_loop<D: MissionDispatch>(
    grant: &AutonomyGrant,
    mut history: DriveHistory,
    instantiation: Option<&Value>,
    dispatcher: &mut D,
) -> Result<DriveOutcome, AutopilotError> {
    let disposition = loop {
        match plan_next_action(grant, &history)? {
            NextAction::DispatchFull { mission, .. } => {
                if grant.require_reconciliation_complete()
                    && !reconciliation_source_exists(&history, instantiation)
                {
                    break FinalDisposition::Exhausted {
                        accounting: reconciliation_unavailable_accounting(grant, &history),
                    };
                }
                dispatch_once(AttemptKind::Full, mission, instantiation, dispatcher, &mut history)?;
            }
            NextAction::DispatchRepair { mission, .. } => {
                dispatch_once(
                    AttemptKind::Repair,
                    mission,
                    instantiation,
                    dispatcher,
                    &mut history,
                )?;
            }
            NextAction::StopSuccess { evidence } => {
                break FinalDisposition::Succeeded { evidence };
            }
            NextAction::StopExhausted { accounting } => {
                break FinalDisposition::Exhausted { accounting };
            }
            NextAction::StopRefused {
                first_terminal_refusal,
            } => {
                break FinalDisposition::Refused {
                    first_terminal_refusal,
                };
            }
        }
    };
    let report = build_autopilot_report(grant, &history, &disposition)?;
    Ok(DriveOutcome {
        final_status: disposition.status(),
        report,
    })
}

fn dispatch_once<D: MissionDispatch>(
    kind: AttemptKind,
    mission: Value,
    instantiation: Option<&Value>,
    dispatcher: &mut D,
    history: &mut DriveHistory,
) -> Result<(), AutopilotError> {
    let attempt = match dispatcher.dispatch(&mission) {
        Ok(report) => {
            let (reconciliation, note) =
                reconciliation_for_attempt(kind, &mission, &report, instantiation);
            AttemptRecord::delivered(kind, mission, report, reconciliation, note)?
        }
        Err(error) => AttemptRecord::undelivered(kind, mission, error)?,
    };
    history.push(attempt);
    Ok(())
}

/// Drive a bare mission document to a stop state. Reconciliation is available only through the
/// mission's own workflow binding on this path.
pub fn drive_mission<D: MissionDispatch>(
    grant: &AutonomyGrant,
    base_mission: Value,
    dispatcher: &mut D,
) -> Result<DriveOutcome, AutopilotError> {
    let history = DriveHistory::new(base_mission)?;
    drive_loop(grant, history, None, dispatcher)
}

/// Drive the mission carried by a workflow instantiation artifact.
///
/// The instantiation must be an accepted `domain_workflow_instantiate` report; its `mission` is
/// the base mission, and the artifact itself is retained for full-dispatch reconciliation when
/// the mission carries no binding of its own.
pub fn drive_instantiation<D: MissionDispatch>(
    grant: &AutonomyGrant,
    instantiation: &Value,
    dispatcher: &mut D,
) -> Result<DriveOutcome, AutopilotError> {
    let base_mission = instantiation_mission(instantiation)?;
    let history = DriveHistory::new(base_mission)?;
    drive_loop(grant, history, Some(instantiation), dispatcher)
}

/// Extract and check the mission from an instantiation artifact, refusing artifacts that are
/// not accepted instantiation reports so a rejected instantiation cannot be driven anyway.
pub fn instantiation_mission(instantiation: &Value) -> Result<Value, AutopilotError> {
    let object = instantiation
        .as_object()
        .ok_or_else(|| AutopilotError::InvalidInstantiation {
            reason: "instantiation must be a JSON object".into(),
        })?;
    if object.get("workflow").and_then(Value::as_str) != Some("domain_workflow_instantiate") {
        return Err(AutopilotError::InvalidInstantiation {
            reason: "workflow must be domain_workflow_instantiate".into(),
        });
    }
    if object.get("ok").and_then(Value::as_bool) != Some(true) {
        return Err(AutopilotError::InvalidInstantiation {
            reason: "instantiation must be an accepted workflow report (ok: true)".into(),
        });
    }
    object
        .get("mission")
        .filter(|mission| mission.is_object())
        .cloned()
        .ok_or_else(|| AutopilotError::InvalidInstantiation {
            reason: "instantiation.mission must be an object".into(),
        })
}
