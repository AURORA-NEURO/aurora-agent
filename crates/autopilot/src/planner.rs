//! The pure planner: grant plus history in, exactly one next action out.
//!
//! [`plan_next_action`] is deterministic — no clock, no randomness, iteration in the mission
//! plan's own topological order — and total over its inputs: every history is answered with a
//! dispatch or a stop, and every stop carries its accounting. The drive loop is nothing but
//! "call the planner, obey it".
//!
//! # The success rule, stated once
//!
//! [`NextAction::StopSuccess`] requires all of:
//!
//! 1. every step of the base mission has a recorded `succeeded` result in some attempt (the most
//!    recent attempt that dispatched the step decides);
//! 2. the latest attempt's own mission report has `mission_status == "succeeded"`;
//! 3. when the grant requires reconciliation: the latest attempt carries a reconciliation record
//!    whose completion is `complete` **and** whose integrity is valid, in that attempt's own
//!    scope — the full plan for a full dispatch, the re-dispatched subset for a repair;
//! 4. no retained `succeeded` result contradicts the mission's own output budgets. The executor
//!    refuses an over-budget reply instead of truncating it, so a success row measured above the
//!    per-step budget — or a report whose accumulated bytes exceed the total budget — describes
//!    an outcome the mission contract excludes, and success is never read out of it.
//!
//! Nothing is inferred: each requirement reads a retained record.
//!
//! # The repair rule
//!
//! Overall success needs *every* step succeeded, so a repair is dispatched only when every
//! not-yet-succeeded step can be included: failed steps whose 40.36 class the grant retries,
//! plus blocked steps (they never ran). A step is excluded — with the reason recorded — when its
//! recorded outcome is a cancellation (never re-dispatched, whatever the retry options say and
//! whatever the surrounding report's mission status), when its class is not authorised, when a
//! binding from an already-succeeded dependency cannot be re-materialized from the retained
//! payload, or when it depends on an excluded step. Exclusion
//! is permanent under a fixed grant and fixed retained evidence (nothing a repair does can
//! change either), so when any needed step is excluded the planner stops with the full
//! accounting instead of spending side effects on a repair that cannot reach success. This is
//! deliberately stricter than retrying the includable remainder: dispatching work that provably
//! cannot lead to the drive's success criterion is waste dressed as diligence.
//!
//! # Exhaustion is unconstructable, not just unreturned
//!
//! The dispatch variants carry a [`DispatchAuthorization`] whose only constructor is private to
//! this module and gated on `dispatches_used < max_attempts`. Code outside the planner cannot
//! build a dispatch action at all, exhausted history or not:
//!
//! ```compile_fail
//! let forged = bioprism_autopilot::DispatchAuthorization { attempt_index: 99 };
//! ```

use crate::classify::{
    classify_missing_step_result, classify_step_result, RetryClass, StepClass, StepClassification,
};
use crate::error::AutopilotError;
use crate::grant::AutonomyGrant;
use crate::history::DriveHistory;
use bioprism_devplat::{
    apply_binding, plan_mission, MissionError, MissionPolicy, MissionReport, MissionRequest,
    MissionStepResult,
};
use bioprism_ids::ContentHash;
use serde_json::{json, Map, Value};
use std::collections::{BTreeMap, BTreeSet};

/// Proof that the grant's attempt budget authorised one more dispatch when this action was
/// planned. Private field, no public constructor: [`plan_next_action`] is the only mint, and it
/// refuses once `dispatches_used >= max_attempts`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DispatchAuthorization {
    attempt_index: usize,
}

impl DispatchAuthorization {
    /// The 1-based attempt this authorisation covers.
    pub fn attempt_index(&self) -> usize {
        self.attempt_index
    }
}

fn authorize(grant: &AutonomyGrant, history: &DriveHistory) -> Option<DispatchAuthorization> {
    let used = history.dispatches_used();
    (used < grant.max_attempts()).then_some(DispatchAuthorization {
        attempt_index: used + 1,
    })
}

/// What the drive must do next. Dispatch variants are constructable only by the planner.
#[derive(Debug, Clone, PartialEq)]
pub enum NextAction {
    /// Dispatch the whole base mission, policy overwritten from the grant.
    DispatchFull {
        mission: Value,
        authorization: DispatchAuthorization,
    },
    /// Dispatch the repair subset: every not-yet-succeeded step, with bindings from
    /// already-succeeded dependencies inlined from retained payloads.
    DispatchRepair {
        mission: Value,
        authorization: DispatchAuthorization,
        included_step_ids: Vec<String>,
        rematerialized_bindings: usize,
        /// Claim ids the base mission carried and this repair does not: claim lineage exists
        /// only for attempt 1, and the stripping is disclosed here rather than left silent.
        dropped_claim_ids: Vec<String>,
    },
    /// Every requirement of the success rule holds; `evidence` names the records.
    StopSuccess { evidence: Value },
    /// No further dispatch is authorised or none could change the outcome; `accounting` says
    /// which, per step.
    StopExhausted { accounting: Value },
    /// A step failed terminally; re-dispatching it unchanged would be dishonest, so the drive
    /// stops and reports the first such refusal in plan order.
    StopRefused { first_terminal_refusal: Value },
}

struct Disposition<'h> {
    attempt_index: usize,
    classification: StepClassification,
    result: Option<&'h MissionStepResult>,
}

/// Fold every attempt's recorded results into a latest-wins view per step. A step an attempt
/// dispatched without recording a row is an unknown failure, stated as such rather than skipped.
fn merged_dispositions<'h>(history: &'h DriveHistory) -> BTreeMap<String, Disposition<'h>> {
    let mut merged = BTreeMap::new();
    for (index, attempt) in history.attempts().iter().enumerate() {
        let attempt_index = index + 1;
        for step_id in attempt.dispatched_step_ids() {
            let disposition = match attempt.step_result(&step_id) {
                Some(result) => Disposition {
                    attempt_index,
                    classification: classify_step_result(result),
                    result: Some(result),
                },
                None => Disposition {
                    attempt_index,
                    classification: classify_missing_step_result(&step_id),
                    result: None,
                },
            };
            merged.insert(step_id, disposition);
        }
    }
    merged
}

/// The mission executor derives binding payloads from the retained envelope as the parsed
/// `content[0].text` JSON when present, otherwise the whole envelope; this mirrors that exactly
/// so a re-materialized binding sees the same value the original in-mission binding would have.
fn binding_payload(wire: &Value) -> Value {
    wire.pointer("/result/content/0/text")
        .and_then(Value::as_str)
        .and_then(|text| serde_json::from_str::<Value>(text).ok())
        .unwrap_or_else(|| wire.clone())
}

fn map_mission_error(error: MissionError) -> AutopilotError {
    match error {
        MissionError::ToolNotAllowed { step, tool } => AutopilotError::GrantDoesNotAuthorise {
            reason: format!("step `{step}` uses tool `{tool}`, which the grant does not allow"),
        },
        MissionError::SideEffectsDisallowed { step } => AutopilotError::GrantDoesNotAuthorise {
            reason: format!(
                "step `{step}` carries a confirmation flag but the grant does not allow side \
                 effects"
            ),
        },
        MissionError::MissingAllowList => AutopilotError::GrantDoesNotAuthorise {
            reason: "execution requires a non-empty allow-list".into(),
        },
        other => AutopilotError::InvalidMission {
            reason: other.to_string(),
        },
    }
}

/// The base mission with the grant's authority applied: execution on, allow-list and side-effect
/// posture overwritten. The grant narrows or replaces whatever the mission was authored with; it
/// never widens implicitly because the authored values are discarded entirely.
fn adjusted_request(grant: &AutonomyGrant, base: &MissionRequest) -> MissionRequest {
    let mut request = base.clone();
    request.policy.execute = true;
    request.policy.allowed_tools = grant.allowed_tools().to_vec();
    request.policy.allow_side_effects = grant.allow_side_effects();
    request
}

fn encode_request(request: &MissionRequest) -> Result<Value, AutopilotError> {
    serde_json::to_value(request).map_err(|error| AutopilotError::Canonicalisation {
        reason: error.to_string(),
    })
}

fn result_digest(result: &MissionStepResult) -> Option<String> {
    result
        .wire
        .as_ref()
        .and_then(|wire| ContentHash::of_value(wire).ok())
        .map(|digest| digest.to_string())
}

fn success_evidence(
    grant: &AutonomyGrant,
    history: &DriveHistory,
    ordered_steps: &[String],
    merged: &BTreeMap<String, Disposition<'_>>,
    latest_index: usize,
) -> Value {
    let steps = ordered_steps
        .iter()
        .map(|step_id| {
            let disposition = &merged[step_id];
            json!({
                "step_id": step_id,
                "attempt_index": disposition.attempt_index,
                "status": "succeeded",
                "result_digest": disposition.result.and_then(result_digest),
                "arguments_digest": disposition
                    .result
                    .and_then(|result| result.arguments_digest.clone()),
            })
        })
        .collect::<Vec<_>>();
    let reconciliation = if grant.require_reconciliation_complete() {
        let latest = &history.attempts()[latest_index - 1];
        let (status, integrity_valid, digest) = latest
            .reconciliation_summary()
            .expect("success under a reconciliation-requiring grant was checked before this");
        json!({
            "required": true,
            "attempt_index": latest_index,
            "status": status,
            "integrity_valid": integrity_valid,
            "digest": digest,
            "scope": latest.kind().reconciliation_scope(),
        })
    } else {
        json!({
            "required": false,
            "note": "the grant waived the reconciliation-complete requirement",
        })
    };
    json!({
        "mission_status": "succeeded",
        "steps": steps,
        "reconciliation": reconciliation,
    })
}

fn exhausted(
    grant: &AutonomyGrant,
    history: &DriveHistory,
    reason: &str,
    detail: String,
    unresolved: Vec<Value>,
) -> NextAction {
    NextAction::StopExhausted {
        accounting: json!({
            "reason": reason,
            "detail": detail,
            "attempts_used": history.dispatches_used(),
            "max_attempts": grant.max_attempts(),
            "unresolved_steps": unresolved,
        }),
    }
}

fn unresolved_rows(
    ordered_steps: &[String],
    merged: &BTreeMap<String, Disposition<'_>>,
    exclusions: &BTreeMap<String, String>,
) -> Vec<Value> {
    ordered_steps
        .iter()
        .filter_map(|step_id| {
            let disposition = merged.get(step_id);
            let state = disposition
                .map(|d| d.classification.class)
                .unwrap_or(StepClass::Failed(RetryClass::Unknown));
            if matches!(state, StepClass::Succeeded) {
                return None;
            }
            let (signal, reason, attempt_index) = match disposition {
                Some(d) => (
                    d.classification.signal,
                    d.classification.reason.clone(),
                    Some(d.attempt_index),
                ),
                None => (
                    "unrecognised_status",
                    "never dispatched in any attempt".to_string(),
                    None,
                ),
            };
            Some(json!({
                "step_id": step_id,
                "state": state.as_str(),
                "signal": signal,
                "reason": reason,
                "attempt_index": attempt_index,
                "exclusion": exclusions.get(step_id),
            }))
        })
        .collect()
}

/// The unresolved accounting for a mission no attempt has touched: every step, stated as never
/// dispatched. Used by the drive's pre-dispatch refusal so its accounting carries the same rows
/// the planner's own stops carry.
pub(crate) fn never_dispatched_rows(ordered_steps: &[String]) -> Vec<Value> {
    unresolved_rows(ordered_steps, &BTreeMap::new(), &BTreeMap::new())
}

fn grant_retries(grant: &AutonomyGrant, class: RetryClass) -> bool {
    match class {
        RetryClass::Terminal => false,
        RetryClass::RetryableAsIs => grant.retry().retryable_as_is(),
        RetryClass::RetryableAfterChange => grant.retry().retryable_after_change(),
        RetryClass::Unknown => grant.retry().unknown(),
    }
}

/// Filter the workflow binding's evidence plan to the repair subset and restore the digest
/// contract the mission validator enforces. Entries are copied verbatim; only membership
/// changes, and the resulting reconciliation is honestly scoped to the subset it covers.
fn repair_binding(
    binding: &Value,
    included: &BTreeSet<String>,
) -> Result<Option<Value>, AutopilotError> {
    let object = binding
        .as_object()
        .ok_or_else(|| AutopilotError::InvalidMission {
            reason: "workflow_binding must be an object".into(),
        })?;
    let plan = object
        .get("evidence_plan")
        .and_then(Value::as_object)
        .ok_or_else(|| AutopilotError::InvalidMission {
            reason: "workflow_binding.evidence_plan must be an object".into(),
        })?;
    let steps = plan.get("steps").and_then(Value::as_array).ok_or_else(|| {
        AutopilotError::InvalidMission {
            reason: "workflow_binding.evidence_plan.steps must be an array".into(),
        }
    })?;
    let mut covered = BTreeSet::new();
    let filtered = steps
        .iter()
        .filter(|entry| {
            entry
                .get("step_id")
                .and_then(Value::as_str)
                .is_some_and(|id| {
                    if included.contains(id) {
                        covered.insert(id.to_string());
                        true
                    } else {
                        false
                    }
                })
        })
        .cloned()
        .collect::<Vec<_>>();
    if covered.len() != included.len() {
        return Ok(None);
    }
    let mut new_plan = Map::new();
    for (key, value) in plan {
        if key == "steps" {
            new_plan.insert(key.clone(), Value::Array(filtered.clone()));
        } else {
            new_plan.insert(key.clone(), value.clone());
        }
    }
    let new_plan = Value::Object(new_plan);
    let digest = ContentHash::of_value(&new_plan)
        .map_err(|error| AutopilotError::Canonicalisation {
            reason: error.to_string(),
        })?
        .to_string();
    let mut new_binding = object.clone();
    new_binding.insert("evidence_plan".into(), new_plan);
    new_binding.insert("evidence_plan_digest".into(), Value::String(digest));
    Ok(Some(Value::Object(new_binding)))
}

/// Rows describing retained results that contradict the mission's own output budgets.
///
/// The mission executor measures every nested reply and refuses the step when the reply exceeds
/// the per-step budget, so a retained `succeeded` row whose byte count is above that budget
/// describes an outcome the mission contract says cannot exist. The same holds for a report whose
/// accumulated `returned_bytes` is above the total budget. Either shape is what a truncating or
/// budget-ignoring executor would produce while still reporting success, and success claimed from
/// it would be success inferred rather than read. The grant does not carry output budgets, so the
/// mission's own policy is the authority these rows are checked against.
fn output_budget_contradictions(
    policy: &MissionPolicy,
    merged: &BTreeMap<String, Disposition<'_>>,
    latest_report: &MissionReport,
) -> Vec<Value> {
    let mut rows = Vec::new();
    for (step_id, disposition) in merged {
        if !matches!(disposition.classification.class, StepClass::Succeeded) {
            continue;
        }
        let Some(result) = disposition.result else {
            continue;
        };
        if result.bytes > policy.max_step_output_bytes {
            rows.push(json!({
                "step_id": step_id,
                "attempt_index": disposition.attempt_index,
                "kind": "step_output_budget",
                "recorded_bytes": result.bytes,
                "budget": policy.max_step_output_bytes,
            }));
        }
    }
    if latest_report.returned_bytes > policy.max_total_output_bytes {
        rows.push(json!({
            "step_id": Value::Null,
            "kind": "total_output_budget",
            "recorded_bytes": latest_report.returned_bytes,
            "budget": policy.max_total_output_bytes,
        }));
    }
    rows
}

/// Decide the next action. Pure and deterministic; see the module documentation for the success,
/// repair, and stop rules this function implements.
pub fn plan_next_action(
    grant: &AutonomyGrant,
    history: &DriveHistory,
) -> Result<NextAction, AutopilotError> {
    let adjusted = adjusted_request(grant, history.parsed_base());
    let plan = plan_mission(&adjusted).map_err(map_mission_error)?;

    if history.attempts().is_empty() {
        let authorization = authorize(grant, history).expect("a grant carries at least 1 attempt");
        return Ok(NextAction::DispatchFull {
            mission: encode_request(&adjusted)?,
            authorization,
        });
    }

    let latest_index = history.dispatches_used();
    let latest = history.latest().expect("attempts checked non-empty");
    let merged = merged_dispositions(history);
    if let Some(error) = latest.dispatch_error() {
        return Ok(exhausted(
            grant,
            history,
            "dispatch_transport_error",
            format!(
                "the dispatch returned no mission report ({error}); the mission outcome is \
                 unknown at mission level and side effects may have run, so the drive stops \
                 rather than re-sending blind"
            ),
            unresolved_rows(&plan.ordered_steps, &merged, &BTreeMap::new()),
        ));
    }
    let latest_report = latest
        .parsed_report()
        .expect("a delivered attempt carries a parsed report");
    if latest_report.mission_status == "cancelled" {
        return Ok(exhausted(
            grant,
            history,
            "mission_cancelled",
            "the latest mission report records an operator cancellation; the autopilot never \
             re-dispatches past a cancellation, whatever the grant's retry options say"
                .into(),
            unresolved_rows(&plan.ordered_steps, &merged, &BTreeMap::new()),
        ));
    }

    let all_succeeded = plan.ordered_steps.iter().all(|step_id| {
        merged
            .get(step_id)
            .is_some_and(|d| matches!(d.classification.class, StepClass::Succeeded))
    });

    if all_succeeded && latest_report.mission_status == "succeeded" {
        let contradictions =
            output_budget_contradictions(&history.parsed_base().policy, &merged, latest_report);
        if !contradictions.is_empty() {
            return Ok(exhausted(
                grant,
                history,
                "output_budget_contradiction",
                format!(
                    "{} retained result(s) claim success while breaching the mission's own \
                     output budgets, which the executor refuses rather than truncates; a report \
                     that reports success over a budget it did not honour is not evidence of \
                     success: {}",
                    contradictions.len(),
                    Value::Array(contradictions.clone())
                ),
                contradictions,
            ));
        }
        let reconciliation_ok = if grant.require_reconciliation_complete() {
            matches!(
                latest.reconciliation_summary(),
                Some((status, true, _)) if status == "complete"
            )
        } else {
            true
        };
        if reconciliation_ok {
            return Ok(NextAction::StopSuccess {
                evidence: success_evidence(
                    grant,
                    history,
                    &plan.ordered_steps,
                    &merged,
                    latest_index,
                ),
            });
        }
        let detail = match latest.reconciliation_summary() {
            Some((status, integrity_valid, _)) => format!(
                "every step succeeded but the latest reconciliation records completion \
                 `{status}` with integrity_valid={integrity_valid}; the grant requires \
                 `complete` with valid integrity, and re-dispatching succeeded steps to \
                 manufacture one would re-run side effects"
            ),
            None => format!(
                "every step succeeded but no reconciliation record accompanies the latest \
                 attempt ({}); the grant requires one and success is never inferred",
                latest
                    .reconciliation_note()
                    .unwrap_or("no reason was recorded")
            ),
        };
        return Ok(exhausted(
            grant,
            history,
            "reconciliation_incomplete",
            detail,
            unresolved_rows(&plan.ordered_steps, &merged, &BTreeMap::new()),
        ));
    }

    for step_id in &plan.ordered_steps {
        let Some(disposition) = merged.get(step_id) else {
            continue;
        };
        if matches!(
            disposition.classification.class,
            StepClass::Failed(RetryClass::Terminal)
        ) {
            let error = disposition.result.and_then(|result| result.error.clone());
            let tool = disposition
                .result
                .map(|result| result.tool.clone())
                .unwrap_or_default();
            return Ok(NextAction::StopRefused {
                first_terminal_refusal: json!({
                    "step_id": step_id,
                    "tool": tool,
                    "attempt_index": disposition.attempt_index,
                    "status": disposition.classification.status,
                    "signal": disposition.classification.signal,
                    "reason": disposition.classification.reason,
                    "error": error,
                }),
            });
        }
    }

    let needed = plan
        .ordered_steps
        .iter()
        .filter(|step_id| {
            !merged
                .get(*step_id)
                .is_some_and(|d| matches!(d.classification.class, StepClass::Succeeded))
        })
        .cloned()
        .collect::<Vec<_>>();
    if needed.is_empty() {
        return Ok(exhausted(
            grant,
            history,
            "inconsistent_report",
            "every step succeeded but the latest mission report does not record mission-level \
             success; nothing is honestly re-dispatchable"
                .into(),
            unresolved_rows(&plan.ordered_steps, &merged, &BTreeMap::new()),
        ));
    }

    if history.dispatches_used() >= grant.max_attempts() {
        let rows = unresolved_rows(&plan.ordered_steps, &merged, &BTreeMap::new());
        return Ok(exhausted(
            grant,
            history,
            "attempt_budget_exhausted",
            format!(
                "{} of {} authorised dispatches are used and {} steps remain unresolved",
                history.dispatches_used(),
                grant.max_attempts(),
                needed.len()
            ),
            rows,
        ));
    }

    if grant.require_reconciliation_complete() && history.parsed_base().workflow_binding.is_none() {
        let rows = unresolved_rows(&plan.ordered_steps, &merged, &BTreeMap::new());
        return Ok(exhausted(
            grant,
            history,
            "repair_reconciliation_unavailable",
            "the base mission carries no workflow binding, so a repair dispatch could never \
             carry the subset-scoped reconciliation the grant requires; dispatching it would \
             spend side effects on an attempt that cannot reach success"
                .into(),
            rows,
        ));
    }

    let mut included: BTreeSet<String> = BTreeSet::new();
    let mut exclusions: BTreeMap<String, String> = BTreeMap::new();
    let base_steps: BTreeMap<&str, _> = history
        .parsed_base()
        .steps
        .iter()
        .map(|step| (step.id.as_str(), step))
        .collect();
    for step_id in &needed {
        let step = base_steps[step_id.as_str()];
        let disposition = merged.get(step_id);
        let class = disposition
            .map(|d| d.classification.class)
            .unwrap_or(StepClass::Failed(RetryClass::Unknown));
        if disposition.is_some_and(|d| d.classification.signal == "cancelled") {
            exclusions.insert(
                step_id.clone(),
                "the step was cancelled; a cancellation is an authority the drive never retries \
                 past, whatever the grant's retry options say"
                    .into(),
            );
            continue;
        }
        match class {
            StepClass::Succeeded => unreachable!("needed steps are the not-succeeded steps"),
            StepClass::Blocked => {}
            StepClass::Failed(retry) => {
                if !grant_retries(grant, retry) {
                    exclusions.insert(
                        step_id.clone(),
                        format!(
                            "retry of class `{}` is not authorised by the grant",
                            retry.as_str()
                        ),
                    );
                    continue;
                }
            }
        }
        let mut excluded_reason = None;
        for dependency in &step.depends_on {
            let dependency_succeeded = merged
                .get(dependency)
                .is_some_and(|d| matches!(d.classification.class, StepClass::Succeeded));
            if dependency_succeeded || included.contains(dependency) {
                continue;
            }
            excluded_reason = Some(format!(
                "depends on `{dependency}`, which is excluded from this repair"
            ));
            break;
        }
        if excluded_reason.is_none() {
            for binding in &step.bindings {
                let source_succeeded = merged
                    .get(&binding.from_step)
                    .is_some_and(|d| matches!(d.classification.class, StepClass::Succeeded));
                if !source_succeeded {
                    continue;
                }
                let wire = merged
                    .get(&binding.from_step)
                    .and_then(|d| d.result)
                    .and_then(|result| result.wire.as_ref());
                match wire {
                    None => {
                        excluded_reason = Some(format!(
                            "binding from `{}` cannot be re-materialized: the succeeded result's \
                             output was not retained",
                            binding.from_step
                        ));
                        break;
                    }
                    Some(wire) => {
                        let payload = binding_payload(wire);
                        let resolvable = binding.source_pointer.is_empty()
                            || payload.pointer(&binding.source_pointer).is_some();
                        if !resolvable {
                            excluded_reason = Some(format!(
                                "binding from `{}` cannot be re-materialized: source pointer \
                                 `{}` is missing from the retained payload",
                                binding.from_step, binding.source_pointer
                            ));
                            break;
                        }
                    }
                }
            }
        }
        match excluded_reason {
            Some(reason) => {
                exclusions.insert(step_id.clone(), reason);
            }
            None => {
                included.insert(step_id.clone());
            }
        }
    }

    if !exclusions.is_empty() {
        let rows = unresolved_rows(&plan.ordered_steps, &merged, &exclusions);
        return Ok(exhausted(
            grant,
            history,
            "unresolved_steps_not_retryable",
            format!(
                "{} of {} unresolved steps cannot be included in a repair under this grant and \
                 this retained evidence, and overall success requires every step; exclusion \
                 reasons are recorded per step",
                exclusions.len(),
                needed.len()
            ),
            rows,
        ));
    }

    let attempt_index = history.dispatches_used() + 1;
    let mut rematerialized = 0usize;
    let mut repair_steps = Vec::with_capacity(included.len());
    for step in &history.parsed_base().steps {
        if !included.contains(&step.id) {
            continue;
        }
        let mut new_step = step.clone();
        let mut kept_bindings = Vec::new();
        for binding in &step.bindings {
            if included.contains(&binding.from_step) {
                kept_bindings.push(binding.clone());
                continue;
            }
            let wire = merged
                .get(&binding.from_step)
                .and_then(|d| d.result)
                .and_then(|result| result.wire.as_ref())
                .expect("materializability was proven during inclusion");
            let payload = binding_payload(wire);
            apply_binding(&mut new_step.arguments, binding, &payload).map_err(map_mission_error)?;
            rematerialized += 1;
        }
        new_step.bindings = kept_bindings;
        new_step
            .depends_on
            .retain(|dependency| included.contains(dependency));
        repair_steps.push(new_step);
    }

    let mut repair = adjusted_request(grant, history.parsed_base());
    repair.mission_id = format!(
        "{}-repair-{}",
        history.parsed_base().mission_id,
        attempt_index
    );
    repair.goal = format!(
        "{} (autopilot repair attempt {attempt_index}: re-dispatching {} unresolved steps)",
        history.parsed_base().goal,
        repair_steps.len()
    );
    repair.steps = repair_steps;
    let dropped_claim_ids = repair
        .claim_requests
        .iter()
        .map(|request| request.id.clone())
        .collect::<Vec<_>>();
    repair.claim_requests = Vec::new();
    repair.evaluator_review = None;
    repair.route_review = None;
    repair.workflow_binding = match &history.parsed_base().workflow_binding {
        Some(binding) => match repair_binding(binding, &included)? {
            Some(filtered) => Some(filtered),
            None => {
                let rows = unresolved_rows(&plan.ordered_steps, &merged, &exclusions);
                return Ok(exhausted(
                    grant,
                    history,
                    "evidence_plan_gap",
                    "the workflow binding's evidence plan does not cover every repair step, so \
                     a subset-scoped reconciliation could never be complete"
                        .into(),
                    rows,
                ));
            }
        },
        None => None,
    };
    plan_mission(&repair).map_err(map_mission_error)?;
    let authorization =
        authorize(grant, history).expect("the attempt budget was checked before construction");
    Ok(NextAction::DispatchRepair {
        mission: encode_request(&repair)?,
        authorization,
        included_step_ids: included.iter().cloned().collect(),
        rematerialized_bindings: rematerialized,
        dropped_claim_ids,
    })
}

/// The pure planning of attempt 1 only, for a no-dispatch preview. Identical to
/// [`plan_next_action`] on an empty history; the caller labels the result as no-dispatch and
/// performs zero writes.
pub fn preview_first_action(
    grant: &AutonomyGrant,
    base_mission: Value,
) -> Result<NextAction, AutopilotError> {
    let history = DriveHistory::new(base_mission)?;
    plan_next_action(grant, &history)
}
