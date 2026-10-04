//! The autopilot report: one document chaining every receipt a drive produced.
//!
//! The drive's receipts already exist — mission reports and reconciliation records are produced
//! and retained by the machinery this crate calls. The autopilot report adds the one thing they
//! cannot say individually: which grant authorised the drive, which attempt produced which
//! digest, how each step's outcome classified, and why the drive stopped. Its own digest is
//! computed the way the evidence bundle computes its: over the canonical document with the
//! digest field removed, so any later edit is detectable by recomputation alone.
//!
//! `limitations` is always present and always contains at least [`REQUIRED_LIMITATIONS`]. A
//! report that omitted them would imply recurrence, caller identity, unrestricted resume, or
//! deadline ownership that the drive deliberately does not have.
//!
//! # The classification table is keyed on the dispatch, not on the reply
//!
//! Each attempt's `classification_table` is built by walking the step ids that attempt actually
//! dispatched and looking each one up in the returned report. A dispatched step the report says
//! nothing about is classified as missing rather than dropped, and a result row naming a step the
//! mission never carried is not admitted at all. Keying the table on `report.results` instead
//! would let the answer decide what the question was: the table would understate a dispatch whose
//! reply lost rows and overstate one whose reply invented them, and the planner — which does key
//! on the dispatch — would silently disagree with the receipt describing it.

use crate::classify::{classify_missing_step_result, classify_step_result};
use crate::error::AutopilotError;
use crate::grant::{AutonomyGrant, AutonomyGrantDocument};
use crate::history::{reconciliation_digest_verified, DriveHistory};
use bioprism_ids::ContentHash;
use serde_json::{json, Map, Value};

pub const AUTOPILOT_REPORT_SCHEMA_VERSION: &str = "bioprism-autopilot/report/0.7";

const PREVIOUS_AUTOPILOT_REPORT_SCHEMA_VERSION: &str = "bioprism-autopilot/report/0.6";
const OLDER_AUTOPILOT_REPORT_SCHEMA_VERSION: &str = "bioprism-autopilot/report/0.5";
const OLDEST_AUTOPILOT_REPORT_SCHEMA_VERSION: &str = "bioprism-autopilot/report/0.4";
const ANCIENT_AUTOPILOT_REPORT_SCHEMA_VERSION: &str = "bioprism-autopilot/report/0.3";
const OLDER_ANCIENT_AUTOPILOT_REPORT_SCHEMA_VERSION: &str = "bioprism-autopilot/report/0.2";
const LEGACY_AUTOPILOT_REPORT_SCHEMA_VERSION: &str = "bioprism-autopilot/report/0.1";
const V0_5_REQUIRED_LIMITATIONS: [&str; 6] = REQUIRED_LIMITATIONS;
const V0_6_REQUIRED_LIMITATIONS: [&str; 6] = REQUIRED_LIMITATIONS;
const V0_4_REQUIRED_LIMITATIONS: [&str; 5] = [
    "no recurrence: the drive runs one mission to a stop state and never repeats a completed mission",
    "bounded MCP exposure: the MCP adapter runs one caller-granted drive per invocation and does not own recurrence or caller identity",
    "metadata-only cross-process resume: checkpoints retain digests and bounded status metadata, while callers must rehydrate private mission and report material",
    "wall-clock ownership and deadlines remain caller-owned: a grant may authorize logical-tick retry backoff, but the wait seam and deadline policy live outside the kernel",
    "an invalid mission report is retained by digest and never re-dispatched because its side effects may already have run",
];
const PREVIOUS_REQUIRED_LIMITATIONS: [&str; 4] = [
    "no recurrence: the drive runs one mission to a stop state and never repeats a completed mission",
    "bounded MCP exposure: the MCP adapter runs one caller-granted drive per invocation and does not own recurrence or caller identity",
    "metadata-only cross-process resume: checkpoints retain digests and bounded status metadata, while callers must rehydrate private mission and report material",
    "wall-clock ownership and deadlines remain caller-owned: a grant may authorize logical-tick retry backoff, but the wait seam and deadline policy live outside the kernel",
];
const LEGACY_REQUIRED_LIMITATIONS: [&str; 4] = [
    "no recurrence: the drive runs one mission to a stop state and never repeats a completed mission",
    "no MCP tool exposure: the autopilot is not an MCP tool and registers nothing with the server",
    "metadata-only cross-process resume: checkpoints retain digests and bounded status metadata, while callers must rehydrate private mission and report material",
    "wall-clock ownership and deadlines remain caller-owned: a grant may authorize logical-tick retry backoff, but the wait seam and deadline policy live outside the kernel",
];

/// The limitations every report must carry. Verification refuses a report missing any of them.
pub const REQUIRED_LIMITATIONS: [&str; 6] = [
    "no recurrence: the drive runs one mission to a stop state and never repeats a completed mission",
    "bounded MCP exposure: the MCP adapter runs one caller-granted drive per invocation and does not own recurrence or caller identity",
    "metadata-only cross-process resume: checkpoints retain digests and bounded status metadata, while callers must rehydrate private mission and report material",
    "wall-clock ownership and deadlines remain caller-owned: a grant may authorize logical-tick retry backoff, but the wait seam and deadline policy live outside the kernel",
    "an ambiguous dispatch outcome is reported as outcome_unknown and is never re-dispatched",
    "an invalid mission report is retained by digest and never re-dispatched because its side effects may already have run",
];

const ADDITIONAL_LIMITATIONS: [&str; 4] = [
    "an undelivered dispatch is never re-sent: a missing mission report leaves side effects unknown at mission level",
    "a repair attempt's reconciliation covers only the re-dispatched subset and is labelled with that scope",
    "succeeded steps are never re-dispatched; a binding whose retained payload is gone excludes its dependent instead",
    "repair claim lineage is limited to claims whose complete evidence basis is in the repair subset; prior results and evaluator or route reviews are not merged or reused",
];

/// How a drive ended. An unknown outcome needs external reconciliation; a paused drive is
/// resumable from its caller-retained checkpoint.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FinalStatus {
    Succeeded,
    Exhausted,
    OutcomeUnknown,
    Refused,
    Paused,
}

impl FinalStatus {
    pub const ALL: [FinalStatus; 5] = [
        FinalStatus::Succeeded,
        FinalStatus::Exhausted,
        FinalStatus::OutcomeUnknown,
        FinalStatus::Refused,
        FinalStatus::Paused,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            FinalStatus::Succeeded => "succeeded",
            FinalStatus::Exhausted => "exhausted",
            FinalStatus::OutcomeUnknown => "outcome_unknown",
            FinalStatus::Refused => "refused",
            FinalStatus::Paused => "paused",
        }
    }
}

/// The stop the planner returned, carried into the report with its structured detail.
#[derive(Debug, Clone, PartialEq)]
pub enum FinalDisposition {
    Succeeded { evidence: Value },
    Exhausted { accounting: Value },
    OutcomeUnknown { outcome_unknown: Value },
    Refused { first_terminal_refusal: Value },
    RepairRefused { refusal: Value },
    Paused { continuation: Value },
}

impl FinalDisposition {
    pub fn status(&self) -> FinalStatus {
        match self {
            FinalDisposition::Succeeded { .. } => FinalStatus::Succeeded,
            FinalDisposition::Exhausted { .. } => FinalStatus::Exhausted,
            FinalDisposition::OutcomeUnknown { .. } => FinalStatus::OutcomeUnknown,
            FinalDisposition::Refused { .. } | FinalDisposition::RepairRefused { .. } => {
                FinalStatus::Refused
            }
            FinalDisposition::Paused { .. } => FinalStatus::Paused,
        }
    }

    fn detail(&self) -> (&'static str, &Value) {
        match self {
            FinalDisposition::Succeeded { evidence } => ("evidence", evidence),
            FinalDisposition::Exhausted { accounting } => ("accounting", accounting),
            FinalDisposition::OutcomeUnknown { outcome_unknown } => {
                ("outcome_unknown", outcome_unknown)
            }
            FinalDisposition::Refused {
                first_terminal_refusal,
            } => ("first_terminal_refusal", first_terminal_refusal),
            FinalDisposition::RepairRefused { refusal } => ("repair_refusal", refusal),
            FinalDisposition::Paused { continuation } => ("continuation", continuation),
        }
    }
}

fn attempt_rows(history: &DriveHistory) -> Vec<Value> {
    history
        .attempts()
        .iter()
        .enumerate()
        .map(|(index, attempt)| {
            let classification_table = attempt
                .dispatched_step_ids()
                .iter()
                .map(|step_id| {
                    let row = match attempt.step_result(step_id) {
                        Some(result) => classify_step_result(result),
                        None => classify_missing_step_result(step_id),
                    };
                    json!({
                        "step_id": row.step_id,
                        "status": row.status,
                        "class": row.class.as_str(),
                        "signal": row.signal,
                        "reason": row.reason,
                    })
                })
                .collect::<Vec<_>>();
            let outcome_summary = attempt
                .parsed_report()
                .map(|report| {
                    json!({
                        "mission_status": report.mission_status,
                        "succeeded": report.succeeded,
                        "refused": report.refused,
                        "blocked": report.blocked,
                        "cancelled": report.cancelled,
                        "required_failures": report.required_failures,
                    })
                })
                .unwrap_or(Value::Null);
            let (
                reconciliation_digest,
                reconciliation_status,
                reconciliation_scope,
                reconciliation_digest_verified,
            ) = match attempt.reconciliation_summary() {
                Some((status, integrity_valid, digest, digest_verified)) => (
                    digest.map(Value::String).unwrap_or(Value::Null),
                    json!({ "completion": status, "integrity_valid": integrity_valid }),
                    Value::String(attempt.kind().reconciliation_scope().into()),
                    Value::Bool(digest_verified),
                ),
                None => (Value::Null, Value::Null, Value::Null, Value::Null),
            };
            json!({
                "attempt_index": index + 1,
                "kind": attempt.kind().as_str(),
                "mission_digest": attempt.mission_digest(),
                "dispatched_step_ids": attempt.dispatched_step_ids(),
                "report_digest": attempt.report_digest(),
                "outcome_summary": outcome_summary,
                "classification_table": classification_table,
                "report_validation_error": attempt.report_validation_error(),
                "reconciliation_record": attempt.reconciliation().cloned().unwrap_or(Value::Null),
                "reconciliation_digest": reconciliation_digest,
                "reconciliation_digest_verified": reconciliation_digest_verified,
                "reconciliation_status": reconciliation_status,
                "reconciliation_scope": reconciliation_scope,
                "reconciliation_note": attempt.reconciliation_note(),
                "dispatch_error": attempt.dispatch_error(),
            })
        })
        .collect()
}

fn is_digest(value: &Value) -> bool {
    value
        .as_str()
        .is_some_and(|value| ContentHash::parse(value.to_string()).is_ok())
}

fn number(object: &Map<String, Value>, key: &str) -> Option<u64> {
    object.get(key).and_then(Value::as_u64)
}

fn unknown_unresolved_steps_match_history(
    detail: &Map<String, Value>,
    attempts: &[Value],
    steps_in_plan: Option<u64>,
) -> bool {
    let Some(unresolved) = detail.get("unresolved_steps").and_then(Value::as_array) else {
        return false;
    };
    let Some(first_attempt) = attempts.first() else {
        return false;
    };
    let Some(plan_steps) = first_attempt
        .get("dispatched_step_ids")
        .and_then(Value::as_array)
    else {
        return false;
    };
    if u64::try_from(plan_steps.len()).ok() != steps_in_plan {
        return false;
    }
    let plan_step_ids = plan_steps
        .iter()
        .filter_map(Value::as_str)
        .collect::<Vec<_>>();
    let plan_step_set = plan_step_ids
        .iter()
        .copied()
        .collect::<std::collections::BTreeSet<_>>();
    if plan_step_ids.len() != plan_steps.len() || plan_step_set.len() != plan_step_ids.len() {
        return false;
    }

    for attempt in attempts {
        let Some(dispatched) = attempt.get("dispatched_step_ids").and_then(Value::as_array) else {
            return false;
        };
        if dispatched
            .iter()
            .filter_map(Value::as_str)
            .any(|step_id| !plan_step_set.contains(step_id))
        {
            return false;
        }
    }

    let mut expected = Vec::new();
    for step_id in plan_step_ids {
        let mut latest = None;
        for (attempt_index, attempt) in attempts.iter().enumerate() {
            let Some(dispatched) = attempt.get("dispatched_step_ids").and_then(Value::as_array)
            else {
                return false;
            };
            if dispatched
                .iter()
                .any(|dispatched_id| dispatched_id.as_str() == Some(step_id))
            {
                let Some(row) = attempt
                    .get("classification_table")
                    .and_then(Value::as_array)
                    .and_then(|rows| {
                        rows.iter()
                            .find(|row| row.get("step_id").and_then(Value::as_str) == Some(step_id))
                    })
                else {
                    return false;
                };
                latest = Some((attempt_index + 1, row));
            }
        }
        let Some((attempt_index, classification)) = latest else {
            return false;
        };
        let Some(class) = classification.get("class").and_then(Value::as_str) else {
            return false;
        };
        let status = classification.get("status").and_then(Value::as_str);
        if class == "succeeded" {
            if status != Some("succeeded") {
                return false;
            }
            continue;
        }
        if status == Some("succeeded") {
            return false;
        }
        let (Some(signal), Some(reason)) = (
            classification.get("signal").and_then(Value::as_str),
            classification.get("reason").and_then(Value::as_str),
        ) else {
            return false;
        };
        expected.push(json!({
            "step_id": step_id,
            "state": class,
            "signal": signal,
            "reason": reason,
            "attempt_index": attempt_index,
            "exclusion": null,
        }));
    }
    !expected.is_empty() && expected == *unresolved
}

fn current_report_shape_errors(object: &Map<String, Value>) -> Vec<String> {
    let mut errors = Vec::new();
    let require_nonempty_string = |key: &str, errors: &mut Vec<String>| {
        if !object
            .get(key)
            .and_then(Value::as_str)
            .is_some_and(|value| !value.is_empty())
        {
            errors.push(format!("{key} must be a non-empty string"));
        }
    };
    require_nonempty_string("base_mission_id", &mut errors);
    for key in ["base_mission_digest", "grant_digest"] {
        if !object.get(key).is_some_and(is_digest) {
            errors.push(format!("{key} must be a lowercase SHA-256 digest"));
        }
    }

    let grant = object.get("grant").and_then(Value::as_object);
    let validated_grant = object
        .get("grant")
        .cloned()
        .and_then(|value| serde_json::from_value::<AutonomyGrant>(value).ok());
    if grant.is_none() {
        errors.push("grant must be an object".into());
    } else if validated_grant.is_none() {
        errors.push("grant must be a valid autonomy grant document".into());
    }
    if let Some(grant_digest) = object.get("grant_digest").and_then(Value::as_str) {
        match validated_grant
            .as_ref()
            .and_then(|grant| grant.digest().ok())
        {
            Some(actual) if actual == grant_digest => {}
            _ => errors.push("grant_digest must match the validated embedded grant".into()),
        }
    }

    let totals = object.get("totals").and_then(Value::as_object);
    let (attempts_used, max_attempts, steps_in_plan) = if let Some(totals) = totals {
        let attempts_used = number(totals, "attempts_used");
        let max_attempts = number(totals, "max_attempts");
        let steps_in_plan = number(totals, "steps_in_plan");
        if attempts_used.is_none() {
            errors.push("totals.attempts_used must be a non-negative integer".into());
        }
        if !max_attempts.is_some_and(|value| (1..=16).contains(&value)) {
            errors.push("totals.max_attempts must be between 1 and 16".into());
        }
        if !steps_in_plan.is_some_and(|value| value > 0) {
            errors.push("totals.steps_in_plan must be a positive integer".into());
        }
        if let (Some(max_attempts), Some(grant)) = (max_attempts, grant) {
            if number(grant, "max_attempts") != Some(max_attempts) {
                errors.push("totals.max_attempts must match grant.max_attempts".into());
            }
        }
        (attempts_used, max_attempts, steps_in_plan)
    } else {
        errors.push("totals must be an object".into());
        (None, None, None)
    };

    let attempts = object.get("attempts").and_then(Value::as_array);
    if let Some(attempts) = attempts {
        if attempts_used != u64::try_from(attempts.len()).ok() {
            errors.push("totals.attempts_used must equal the attempts array length".into());
        }
        if let (Some(attempts_used), Some(max_attempts)) = (attempts_used, max_attempts) {
            if attempts_used > max_attempts {
                errors.push("totals.attempts_used exceeds the grant attempt budget".into());
            }
        }
        for (index, attempt) in attempts.iter().enumerate() {
            let Some(attempt) = attempt.as_object() else {
                errors.push(format!("attempts[{index}] must be an object"));
                continue;
            };
            if number(attempt, "attempt_index") != u64::try_from(index + 1).ok() {
                errors.push(format!("attempts[{index}].attempt_index is not contiguous"));
            }
            let expected_kind = if index == 0 { "full" } else { "repair" };
            if attempt.get("kind").and_then(Value::as_str) != Some(expected_kind) {
                errors.push(format!("attempts[{index}].kind must be {expected_kind}"));
            }
            if !attempt.get("mission_digest").is_some_and(is_digest) {
                errors.push(format!("attempts[{index}].mission_digest is malformed"));
            }
            let dispatched = attempt.get("dispatched_step_ids").and_then(Value::as_array);
            let Some(dispatched) = dispatched else {
                errors.push(format!(
                    "attempts[{index}].dispatched_step_ids must be an array"
                ));
                continue;
            };
            let mut dispatched_ids = Vec::with_capacity(dispatched.len());
            for step in dispatched {
                match step.as_str().filter(|step| !step.is_empty()) {
                    Some(step) => dispatched_ids.push(step),
                    None => errors.push(format!(
                        "attempts[{index}].dispatched_step_ids must contain non-empty strings"
                    )),
                }
            }
            let unique_dispatched: std::collections::BTreeSet<_> =
                dispatched_ids.iter().copied().collect();
            if dispatched_ids.is_empty() || unique_dispatched.len() != dispatched_ids.len() {
                errors.push(format!(
                    "attempts[{index}].dispatched_step_ids must be non-empty and unique"
                ));
            }
            let classification = attempt
                .get("classification_table")
                .and_then(Value::as_array);
            match classification {
                Some(rows) if rows.len() == dispatched_ids.len() => {
                    for (row, expected_step) in rows.iter().zip(&dispatched_ids) {
                        let valid_row = row.as_object().is_some_and(|row| {
                            row.get("step_id").and_then(Value::as_str) == Some(*expected_step)
                                && ["status", "class", "signal", "reason"]
                                    .iter()
                                    .all(|key| row.get(*key).and_then(Value::as_str).is_some())
                        });
                        if !valid_row {
                            errors.push(format!(
                                "attempts[{index}].classification_table must describe dispatched steps in order"
                            ));
                            break;
                        }
                    }
                }
                _ => errors.push(format!(
                    "attempts[{index}].classification_table must have one row per dispatched step"
                )),
            }

            let report_digest = attempt.get("report_digest");
            let outcome = attempt.get("outcome_summary");
            let dispatch_error = attempt.get("dispatch_error");
            let report_validation_error = attempt.get("report_validation_error");
            match outcome {
                Some(Value::Null) => {
                    let undelivered = report_digest.is_some_and(Value::is_null)
                        && dispatch_error.and_then(Value::as_str).is_some()
                        && report_validation_error.is_some_and(Value::is_null);
                    let invalid_report = report_digest.is_some_and(is_digest)
                        && dispatch_error.is_some_and(Value::is_null)
                        && report_validation_error
                            .and_then(Value::as_str)
                            .is_some_and(|error| !error.is_empty());
                    if !undelivered && !invalid_report {
                        errors.push(format!(
                            "attempts[{index}] without an outcome must be either undelivered or carry a retained invalid report"
                        ));
                    }
                }
                Some(Value::Object(outcome)) => {
                    if !report_digest.is_some_and(is_digest)
                        || !dispatch_error.is_some_and(Value::is_null)
                        || !report_validation_error.is_some_and(Value::is_null)
                    {
                        errors.push(format!(
                            "attempts[{index}] with an outcome must carry a report digest and no report error"
                        ));
                    }
                    if !outcome
                        .get("mission_status")
                        .and_then(Value::as_str)
                        .is_some()
                        || [
                            "succeeded",
                            "refused",
                            "blocked",
                            "cancelled",
                            "required_failures",
                        ]
                        .iter()
                        .any(|key| number(outcome, key).is_none())
                    {
                        errors.push(format!(
                            "attempts[{index}].outcome_summary has an invalid mission status or count"
                        ));
                    }
                }
                _ => errors.push(format!(
                    "attempts[{index}].outcome_summary must be an object or null"
                )),
            }

            let reconciliation_status = attempt.get("reconciliation_status");
            let scope = attempt.get("reconciliation_scope");
            let digest = attempt.get("reconciliation_digest");
            let digest_verified = attempt.get("reconciliation_digest_verified");
            let reconciliation_record = attempt.get("reconciliation_record");
            let note = attempt.get("reconciliation_note");
            if reconciliation_status.is_some_and(Value::is_null) {
                if !scope.is_some_and(Value::is_null)
                    || !digest.is_some_and(Value::is_null)
                    || !digest_verified.is_some_and(Value::is_null)
                    || !reconciliation_record.is_some_and(Value::is_null)
                    || !note.and_then(Value::as_str).is_some()
                {
                    errors.push(format!(
                        "attempts[{index}] without reconciliation must state a note and no scope, digest, or record"
                    ));
                }
            } else if let Some(summary) = reconciliation_status.and_then(Value::as_object) {
                let expected_scope = if expected_kind == "full" {
                    "full_plan"
                } else {
                    "repair_subset"
                };
                if !summary.get("completion").and_then(Value::as_str).is_some()
                    || !summary
                        .get("integrity_valid")
                        .and_then(Value::as_bool)
                        .is_some()
                    || scope.and_then(Value::as_str) != Some(expected_scope)
                    || !(digest.is_some_and(Value::is_null) || digest.is_some_and(is_digest))
                    || !digest_verified.and_then(Value::as_bool).is_some()
                    || (digest_verified.and_then(Value::as_bool) == Some(true)
                        && !digest.is_some_and(is_digest))
                    || !note.is_some_and(Value::is_null)
                {
                    errors.push(format!(
                        "attempts[{index}] has a malformed reconciliation summary"
                    ));
                }
                let record = reconciliation_record.filter(|record| record.is_object());
                let record_matches = record.is_some_and(|record| {
                    let digest_matches = record.get("reconciliation_digest") == digest;
                    let completion_matches =
                        record.pointer("/completion/status") == summary.get("completion");
                    let integrity_matches =
                        record.pointer("/integrity/valid") == summary.get("integrity_valid");
                    let verification_matches = reconciliation_digest_verified(record, None)
                        == digest_verified.and_then(Value::as_bool).unwrap_or(false);
                    digest_matches
                        && completion_matches
                        && integrity_matches
                        && verification_matches
                });
                if !record_matches {
                    errors.push(format!(
                        "attempts[{index}].reconciliation_record does not verify against its summary and digest"
                    ));
                }
            } else {
                errors.push(format!(
                    "attempts[{index}].reconciliation_status must be an object or null"
                ));
            }
        }
    } else {
        errors.push("attempts must be an array".into());
    }

    let status = object
        .get("final_status")
        .and_then(Value::as_str)
        .unwrap_or("");
    if matches!(
        status,
        "succeeded" | "outcome_unknown" | "refused" | "paused"
    ) && attempts_used == Some(0)
    {
        errors.push(format!(
            "final_status {status} requires at least one dispatched attempt"
        ));
    }
    match status {
        "succeeded" => {
            let evidence = object.get("evidence").and_then(Value::as_object);
            let Some(evidence) = evidence else {
                errors.push("evidence must be an object for a succeeded report".into());
                return errors;
            };
            if evidence.get("mission_status").and_then(Value::as_str) != Some("succeeded") {
                errors.push("evidence.mission_status must be succeeded".into());
            }
            let evidence_steps = evidence.get("steps").and_then(Value::as_array);
            if !evidence_steps.is_some_and(|steps| {
                Some(u64::try_from(steps.len()).unwrap_or(u64::MAX)) == steps_in_plan
            }) {
                errors.push("evidence.steps must cover every planned step".into());
            }
            let mut ids = std::collections::BTreeSet::new();
            if let Some(steps) = evidence_steps {
                for step in steps {
                    let valid = step.as_object().is_some_and(|step| {
                        let id = step.get("step_id").and_then(Value::as_str);
                        let index = step.get("attempt_index").and_then(Value::as_u64);
                        id.is_some_and(|id| !id.is_empty())
                            && index.is_some_and(|index| index > 0 && Some(index) <= attempts_used)
                            && step.get("status").and_then(Value::as_str) == Some("succeeded")
                            && ["result_digest", "arguments_digest"].iter().all(|key| {
                                step.get(*key)
                                    .is_some_and(|value| value.is_null() || is_digest(value))
                            })
                    });
                    if !valid {
                        errors.push("evidence.steps contains a malformed success receipt".into());
                        break;
                    }
                    if let Some(id) = step.get("step_id").and_then(Value::as_str) {
                        if !ids.insert(id) {
                            errors.push("evidence.steps contains duplicate step ids".into());
                            break;
                        }
                        let attempt_index = step
                            .get("attempt_index")
                            .and_then(Value::as_u64)
                            .and_then(|index| usize::try_from(index).ok())
                            .and_then(|index| index.checked_sub(1));
                        let supporting_row = attempt_index
                            .and_then(|index| attempts.and_then(|attempts| attempts.get(index)))
                            .and_then(|attempt| attempt.get("classification_table"))
                            .and_then(Value::as_array)
                            .and_then(|rows| {
                                rows.iter().find(|row| {
                                    row.get("step_id").and_then(Value::as_str) == Some(id)
                                })
                            });
                        if !supporting_row.is_some_and(|row| {
                            row.get("status").and_then(Value::as_str) == Some("succeeded")
                                && row.get("class").and_then(Value::as_str) == Some("succeeded")
                        }) {
                            errors.push(format!(
                                "evidence for step {id:?} is not supported by its cited attempt"
                            ));
                        }
                    }
                }
            }
            let reconciliation = evidence.get("reconciliation").and_then(Value::as_object);
            if let Some(reconciliation) = reconciliation {
                let required = reconciliation.get("required").and_then(Value::as_bool);
                let grant_requires = grant
                    .and_then(|grant| grant.get("require_reconciliation_complete"))
                    .and_then(Value::as_bool);
                if required != grant_requires {
                    errors.push("evidence.reconciliation.required must match the grant".into());
                }
                if required == Some(true) {
                    let index = reconciliation.get("attempt_index").and_then(Value::as_u64);
                    let attempt_kind = index
                        .and_then(|index| usize::try_from(index).ok())
                        .and_then(|index| index.checked_sub(1))
                        .and_then(|index| attempts.and_then(|attempts| attempts.get(index)))
                        .and_then(|attempt| attempt.get("kind"))
                        .and_then(Value::as_str);
                    let expected_scope = match attempt_kind {
                        Some("full") => Some("full_plan"),
                        Some("repair") => Some("repair_subset"),
                        _ => None,
                    };
                    let latest_attempt = attempts.and_then(|attempts| {
                        attempts_used
                            .and_then(|used| usize::try_from(used).ok())
                            .and_then(|used| used.checked_sub(1))
                            .and_then(|index| attempts.get(index))
                    });
                    let attempt_reconciliation = latest_attempt
                        .and_then(|attempt| attempt.get("reconciliation_status"))
                        .and_then(Value::as_object);
                    if index != attempts_used
                        || reconciliation.get("status").and_then(Value::as_str) != Some("complete")
                        || reconciliation
                            .get("integrity_valid")
                            .and_then(Value::as_bool)
                            != Some(true)
                        || reconciliation.get("scope").and_then(Value::as_str) != expected_scope
                        || !reconciliation.get("digest").is_some_and(is_digest)
                        || reconciliation
                            .get("digest_verified")
                            .and_then(Value::as_bool)
                            != Some(true)
                        || attempt_reconciliation.and_then(|value| value.get("completion"))
                            != reconciliation.get("status")
                        || attempt_reconciliation.and_then(|value| value.get("integrity_valid"))
                            != reconciliation.get("integrity_valid")
                        || latest_attempt.and_then(|attempt| attempt.get("reconciliation_digest"))
                            != reconciliation.get("digest")
                        || latest_attempt
                            .and_then(|attempt| attempt.get("reconciliation_digest_verified"))
                            .and_then(Value::as_bool)
                            != Some(true)
                    {
                        errors.push(
                            "required success reconciliation is incomplete or malformed".into(),
                        );
                    }
                } else if required == Some(false)
                    && !reconciliation.get("note").and_then(Value::as_str).is_some()
                {
                    errors.push("a waived reconciliation requirement must carry a note".into());
                }
            } else {
                errors.push("evidence.reconciliation must be an object".into());
            }
        }
        "exhausted" => {
            if let Some(accounting) = object.get("accounting").and_then(Value::as_object) {
                if !accounting.get("reason").and_then(Value::as_str).is_some()
                    || !accounting.get("detail").and_then(Value::as_str).is_some()
                    || number(accounting, "attempts_used") != attempts_used
                    || number(accounting, "max_attempts") != max_attempts
                    || !accounting
                        .get("unresolved_steps")
                        .is_some_and(Value::is_array)
                {
                    errors.push("accounting does not match the exhausted drive".into());
                }
            } else {
                errors.push("accounting must be an object for an exhausted report".into());
            }
        }
        "outcome_unknown" => {
            let detail = object.get("outcome_unknown").and_then(Value::as_object);
            let expected_reason =
                attempts
                    .and_then(|attempts| attempts.last())
                    .and_then(|attempt| {
                        if attempt
                            .get("dispatch_error")
                            .and_then(Value::as_str)
                            .is_some()
                        {
                            Some("dispatch_transport_error")
                        } else if attempt
                            .get("report_validation_error")
                            .and_then(Value::as_str)
                            .is_some()
                        {
                            Some("invalid_mission_report")
                        } else {
                            None
                        }
                    });
            let latest_attempt = attempts.and_then(|attempts| attempts.last());
            let latest_classifications = latest_attempt
                .and_then(|attempt| attempt.get("classification_table"))
                .and_then(Value::as_array);
            let classifications_are_unknown = latest_classifications.is_some_and(|rows| {
                !rows.is_empty()
                    && rows.iter().all(|row| {
                        row.get("status").and_then(Value::as_str) == Some("missing")
                            && row.get("class").and_then(Value::as_str) == Some("unknown")
                    })
            });
            if let Some(detail) = detail {
                if detail.get("reason").and_then(Value::as_str) != expected_reason
                    || !detail.get("detail").and_then(Value::as_str).is_some()
                    || number(detail, "attempts_used") != attempts_used
                    || number(detail, "max_attempts") != max_attempts
                    || !attempts.is_some_and(|attempts| {
                        unknown_unresolved_steps_match_history(detail, attempts, steps_in_plan)
                    })
                    || !latest_attempt
                        .and_then(|attempt| attempt.get("outcome_summary"))
                        .is_some_and(Value::is_null)
                    || !classifications_are_unknown
                {
                    errors.push(
                        "outcome_unknown must match the latest dispatched attempt and its reason"
                            .into(),
                    );
                }
            } else {
                errors
                    .push("outcome_unknown must be an object for an outcome_unknown report".into());
            }
        }
        "refused" => {
            if let Some(refusal) = object
                .get("first_terminal_refusal")
                .and_then(Value::as_object)
            {
                if !["step_id", "tool", "status", "signal", "reason"]
                    .iter()
                    .all(|key| refusal.get(*key).and_then(Value::as_str).is_some())
                    || !refusal
                        .get("attempt_index")
                        .and_then(Value::as_u64)
                        .is_some_and(|index| index > 0 && Some(index) <= attempts_used)
                    || !(refusal.get("error").is_some_and(Value::is_null)
                        || refusal.get("error").and_then(Value::as_str).is_some())
                {
                    errors.push("first_terminal_refusal is malformed".into());
                }
            } else if let Some(refusal) = object.get("repair_refusal").and_then(Value::as_object) {
                if refusal.get("reason").and_then(Value::as_str) != Some("repair_plan_refused")
                    || !["error_class", "detail"]
                        .iter()
                        .all(|key| refusal.get(*key).and_then(Value::as_str).is_some())
                    || number(refusal, "attempts_used") != attempts_used
                    || number(refusal, "max_attempts") != max_attempts
                    || !refusal.get("unresolved_steps").is_some_and(Value::is_array)
                {
                    errors.push("repair_refusal is malformed".into());
                }
            } else {
                errors
                    .push("a refused report must carry a terminal or repair refusal object".into());
            }
        }
        "paused" => {
            if let Some(continuation) = object.get("continuation").and_then(Value::as_object) {
                if continuation.get("reason").and_then(Value::as_str)
                    != Some("invocation_dispatch_limit")
                    || number(continuation, "attempts_used") != attempts_used
                    || number(continuation, "max_attempts") != max_attempts
                    || number(continuation, "remaining_dispatch_budget")
                        != max_attempts
                            .zip(attempts_used)
                            .map(|(max, used)| max.saturating_sub(used))
                    || continuation.get("resume_required").and_then(Value::as_bool) != Some(true)
                    || continuation
                        .get("private_attempt_material_required")
                        .and_then(Value::as_bool)
                        != Some(true)
                {
                    errors.push("continuation does not match the paused drive".into());
                }
            } else {
                errors.push("continuation must be an object for a paused report".into());
            }
        }
        _ => {}
    }
    errors
}

/// Build the canonical report for a finished drive and stamp its digest.
pub fn build_autopilot_report(
    grant: &AutonomyGrant,
    history: &DriveHistory,
    disposition: &FinalDisposition,
) -> Result<Value, AutopilotError> {
    let grant_document = AutonomyGrantDocument::from(grant.clone());
    let grant_value =
        serde_json::to_value(grant_document).map_err(|error| AutopilotError::Canonicalisation {
            reason: error.to_string(),
        })?;
    let steps_in_plan = history.parsed_base().steps.len();
    let (detail_key, detail_value) = disposition.detail();
    let mut limitations: Vec<Value> = REQUIRED_LIMITATIONS
        .iter()
        .map(|text| Value::String((*text).into()))
        .collect();
    limitations.extend(
        ADDITIONAL_LIMITATIONS
            .iter()
            .map(|text| Value::String((*text).into())),
    );
    let mut report = json!({
        "schema": AUTOPILOT_REPORT_SCHEMA_VERSION,
        "grant_digest": grant.digest()?,
        "grant": grant_value,
        "base_mission_id": history.parsed_base().mission_id,
        "base_mission_digest": ContentHash::of_value(history.base_mission())
            .map_err(|error| AutopilotError::Canonicalisation { reason: error.to_string() })?
            .to_string(),
        "attempts": attempt_rows(history),
        "final_status": disposition.status().as_str(),
        "totals": {
            "attempts_used": history.dispatches_used(),
            "max_attempts": grant.max_attempts(),
            "steps_in_plan": steps_in_plan,
        },
        "limitations": limitations,
    });
    report[detail_key] = detail_value.clone();
    let digest = ContentHash::of_value(&report)
        .map_err(|error| AutopilotError::Canonicalisation {
            reason: error.to_string(),
        })?
        .to_string();
    report["report_sha256"] = Value::String(digest);
    Ok(report)
}

/// Recompute the digest and check the structural contract of one autopilot report.
///
/// Returns a verification projection rather than a bare boolean so a caller can print exactly
/// which check failed; `valid` is the conjunction. A report that is not even an object, or that
/// claims a different schema, is an error rather than an invalid verification, because there is
/// no autopilot report to verify.
///
/// A claimed `report_sha256` that is not a 64-character lowercase hex digest fails as
/// `digest_malformed`, distinctly from `digest_match`: a shape defect in the claimed digest is
/// not evidence of tampering, and the projection never reports it as one.
pub fn verify_autopilot_report(report: &Value) -> Result<Value, AutopilotError> {
    let object = report
        .as_object()
        .ok_or_else(|| AutopilotError::InvalidAutopilotReport {
            reason: "report must be a JSON object".into(),
        })?;
    let schema = object.get("schema").and_then(Value::as_str).unwrap_or("");
    let required_limitations: &[&str] = match schema {
        AUTOPILOT_REPORT_SCHEMA_VERSION => &REQUIRED_LIMITATIONS,
        PREVIOUS_AUTOPILOT_REPORT_SCHEMA_VERSION => &V0_6_REQUIRED_LIMITATIONS,
        OLDER_AUTOPILOT_REPORT_SCHEMA_VERSION => &V0_5_REQUIRED_LIMITATIONS,
        OLDEST_AUTOPILOT_REPORT_SCHEMA_VERSION => &V0_4_REQUIRED_LIMITATIONS,
        ANCIENT_AUTOPILOT_REPORT_SCHEMA_VERSION | OLDER_ANCIENT_AUTOPILOT_REPORT_SCHEMA_VERSION => {
            &PREVIOUS_REQUIRED_LIMITATIONS
        }
        LEGACY_AUTOPILOT_REPORT_SCHEMA_VERSION => &LEGACY_REQUIRED_LIMITATIONS,
        _ => {
            return Err(AutopilotError::InvalidAutopilotReport {
                reason: format!(
                    "schema is {schema:?}, expected {AUTOPILOT_REPORT_SCHEMA_VERSION:?}, {PREVIOUS_AUTOPILOT_REPORT_SCHEMA_VERSION:?}, {OLDER_AUTOPILOT_REPORT_SCHEMA_VERSION:?}, {OLDEST_AUTOPILOT_REPORT_SCHEMA_VERSION:?}, {ANCIENT_AUTOPILOT_REPORT_SCHEMA_VERSION:?}, {OLDER_ANCIENT_AUTOPILOT_REPORT_SCHEMA_VERSION:?}, or {LEGACY_AUTOPILOT_REPORT_SCHEMA_VERSION:?}"
                ),
            });
        }
    };
    let claimed = object
        .get("report_sha256")
        .and_then(Value::as_str)
        .ok_or_else(|| AutopilotError::InvalidAutopilotReport {
            reason: "report_sha256 must be a string".into(),
        })?
        .to_string();
    let mut without_digest = report.clone();
    without_digest
        .as_object_mut()
        .expect("object checked above")
        .remove("report_sha256");
    let recomputed = ContentHash::of_value(&without_digest)
        .map_err(|error| AutopilotError::Canonicalisation {
            reason: error.to_string(),
        })?
        .to_string();
    let digest_malformed = ContentHash::parse(claimed.clone()).is_err();
    let digest_match = !digest_malformed && claimed == recomputed;

    let limitation_texts = object
        .get("limitations")
        .and_then(Value::as_array)
        .map(|values| {
            values
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let missing_limitations = required_limitations
        .iter()
        .filter(|required| !limitation_texts.iter().any(|text| text == *required))
        .map(|required| Value::String((*required).into()))
        .collect::<Vec<_>>();
    let limitations_present = !limitation_texts.is_empty() && missing_limitations.is_empty();

    let final_status = object
        .get("final_status")
        .and_then(Value::as_str)
        .unwrap_or("");
    let supports_unknown_outcome = schema == AUTOPILOT_REPORT_SCHEMA_VERSION
        || schema == PREVIOUS_AUTOPILOT_REPORT_SCHEMA_VERSION
        || schema == OLDER_AUTOPILOT_REPORT_SCHEMA_VERSION;
    let final_status_known = if supports_unknown_outcome {
        FinalStatus::ALL
            .iter()
            .any(|status| status.as_str() == final_status)
    } else {
        matches!(
            final_status,
            "succeeded" | "exhausted" | "refused" | "paused"
        )
    };
    let attempts_present = object.get("attempts").is_some_and(Value::is_array);
    let final_detail_valid =
        if supports_unknown_outcome || schema == OLDEST_AUTOPILOT_REPORT_SCHEMA_VERSION {
            let detail_keys: &[&str] = if supports_unknown_outcome {
                &[
                    "evidence",
                    "accounting",
                    "outcome_unknown",
                    "first_terminal_refusal",
                    "repair_refusal",
                    "continuation",
                ]
            } else {
                &[
                    "evidence",
                    "accounting",
                    "first_terminal_refusal",
                    "repair_refusal",
                    "continuation",
                ]
            };
            let present_details = detail_keys
                .iter()
                .filter(|key| object.contains_key(**key))
                .count();
            present_details == 1
                && match final_status {
                    "succeeded" => object.contains_key("evidence"),
                    "exhausted" => object.contains_key("accounting"),
                    "outcome_unknown" => {
                        supports_unknown_outcome && object.contains_key("outcome_unknown")
                    }
                    "refused" => {
                        object.contains_key("first_terminal_refusal")
                            || object.contains_key("repair_refusal")
                    }
                    "paused" => object.contains_key("continuation"),
                    _ => false,
                }
        } else {
            true
        };

    let shape_errors = if schema == AUTOPILOT_REPORT_SCHEMA_VERSION {
        current_report_shape_errors(object)
    } else {
        Vec::new()
    };
    let current_schema_shape_valid = if schema == AUTOPILOT_REPORT_SCHEMA_VERSION {
        Some(shape_errors.is_empty())
    } else {
        None
    };
    let reconciliation_digest_verification_supported = schema == AUTOPILOT_REPORT_SCHEMA_VERSION;

    let valid = !digest_malformed
        && digest_match
        && limitations_present
        && final_status_known
        && attempts_present
        && final_detail_valid
        && current_schema_shape_valid != Some(false);
    Ok(json!({
        "schema": schema,
        "valid": valid,
        "digest_malformed": digest_malformed,
        "digest_match": digest_match,
        "claimed_report_sha256": claimed,
        "recomputed_report_sha256": recomputed,
        "limitations_present": limitations_present,
        "missing_limitations": missing_limitations,
        "final_status_known": final_status_known,
        "attempts_present": attempts_present,
        "final_detail_valid": final_detail_valid,
        "current_schema_shape_valid": current_schema_shape_valid,
        "reconciliation_digest_verification_supported": reconciliation_digest_verification_supported,
        "shape_errors": shape_errors,
    }))
}

#[cfg(test)]
mod tests {
    use super::{
        verify_autopilot_report, ANCIENT_AUTOPILOT_REPORT_SCHEMA_VERSION,
        AUTOPILOT_REPORT_SCHEMA_VERSION, LEGACY_AUTOPILOT_REPORT_SCHEMA_VERSION,
        LEGACY_REQUIRED_LIMITATIONS, OLDER_ANCIENT_AUTOPILOT_REPORT_SCHEMA_VERSION,
        OLDER_AUTOPILOT_REPORT_SCHEMA_VERSION, OLDEST_AUTOPILOT_REPORT_SCHEMA_VERSION,
        PREVIOUS_AUTOPILOT_REPORT_SCHEMA_VERSION, PREVIOUS_REQUIRED_LIMITATIONS,
        REQUIRED_LIMITATIONS, V0_4_REQUIRED_LIMITATIONS, V0_5_REQUIRED_LIMITATIONS,
        V0_6_REQUIRED_LIMITATIONS,
    };
    use bioprism_ids::ContentHash;
    use serde_json::{json, Value};

    #[test]
    fn previous_report_schema_0_6_remains_verifiable_with_its_original_limitations() {
        let mut report = json!({
            "schema": PREVIOUS_AUTOPILOT_REPORT_SCHEMA_VERSION,
            "limitations": V0_6_REQUIRED_LIMITATIONS,
            "final_status": "succeeded",
            "attempts": [],
            "evidence": {}
        });
        let digest = ContentHash::of_value(&report).unwrap().to_string();
        report["report_sha256"] = Value::String(digest);

        let verification = verify_autopilot_report(&report).unwrap();
        assert_eq!(
            verification["schema"],
            PREVIOUS_AUTOPILOT_REPORT_SCHEMA_VERSION
        );
        assert_eq!(verification["valid"], true);
        assert_eq!(verification["digest_match"], true);
        assert_eq!(verification["limitations_present"], true);
        assert_eq!(
            verification["reconciliation_digest_verification_supported"],
            false
        );
    }

    #[test]
    fn report_schema_0_4_remains_verifiable_with_its_original_limitations() {
        let mut report = json!({
            "schema": OLDEST_AUTOPILOT_REPORT_SCHEMA_VERSION,
            "limitations": V0_4_REQUIRED_LIMITATIONS,
            "final_status": "succeeded",
            "attempts": [],
            "evidence": {}
        });
        let digest = ContentHash::of_value(&report).unwrap().to_string();
        report["report_sha256"] = Value::String(digest);

        let verification = verify_autopilot_report(&report).unwrap();
        assert_eq!(
            verification["schema"],
            OLDEST_AUTOPILOT_REPORT_SCHEMA_VERSION
        );
        assert_eq!(verification["valid"], true);
        assert_eq!(verification["limitations_present"], true);
    }

    #[test]
    fn report_schema_0_3_remains_verifiable_with_its_original_limitations() {
        let mut report = json!({
            "schema": ANCIENT_AUTOPILOT_REPORT_SCHEMA_VERSION,
            "limitations": PREVIOUS_REQUIRED_LIMITATIONS,
            "final_status": "succeeded",
            "attempts": []
        });
        let digest = ContentHash::of_value(&report).unwrap().to_string();
        report["report_sha256"] = Value::String(digest);

        let verification = verify_autopilot_report(&report).unwrap();
        assert_eq!(
            verification["schema"],
            ANCIENT_AUTOPILOT_REPORT_SCHEMA_VERSION
        );
        assert_eq!(verification["valid"], true);
        assert_eq!(verification["limitations_present"], true);
    }

    #[test]
    fn report_schema_0_2_remains_verifiable_with_its_original_limitations() {
        let mut report = json!({
            "schema": OLDER_ANCIENT_AUTOPILOT_REPORT_SCHEMA_VERSION,
            "limitations": PREVIOUS_REQUIRED_LIMITATIONS,
            "final_status": "succeeded",
            "attempts": []
        });
        let digest = ContentHash::of_value(&report).unwrap().to_string();
        report["report_sha256"] = Value::String(digest);

        let verification = verify_autopilot_report(&report).unwrap();
        assert_eq!(
            verification["schema"],
            OLDER_ANCIENT_AUTOPILOT_REPORT_SCHEMA_VERSION
        );
        assert_eq!(verification["valid"], true);
        assert_eq!(verification["limitations_present"], true);
    }

    #[test]
    fn legacy_report_schema_remains_verifiable_with_its_original_limitations() {
        let mut report = json!({
            "schema": LEGACY_AUTOPILOT_REPORT_SCHEMA_VERSION,
            "limitations": LEGACY_REQUIRED_LIMITATIONS,
            "final_status": "succeeded",
            "attempts": []
        });
        let digest = ContentHash::of_value(&report).unwrap().to_string();
        report["report_sha256"] = Value::String(digest);

        let verification = verify_autopilot_report(&report).unwrap();
        assert_eq!(verification["valid"], true);
        assert_eq!(verification["limitations_present"], true);
    }

    #[test]
    fn current_report_refuses_missing_or_conflicting_final_details() {
        let mut report = valid_refused_report();
        seal(&mut report);
        assert_eq!(verify_autopilot_report(&report).unwrap()["valid"], true);

        let mut missing = report.clone();
        missing
            .as_object_mut()
            .unwrap()
            .remove("first_terminal_refusal");
        seal(&mut missing);
        let verification = verify_autopilot_report(&missing).unwrap();
        assert_eq!(verification["valid"], false);
        assert_eq!(verification["final_detail_valid"], false);

        let mut conflicting = report;
        conflicting["accounting"] = json!({ "reason": "wrong_status_detail" });
        seal(&mut conflicting);
        let verification = verify_autopilot_report(&conflicting).unwrap();
        assert_eq!(verification["valid"], false);
        assert_eq!(verification["final_detail_valid"], false);
    }

    #[test]
    fn current_report_requires_its_grant_mission_and_consistent_totals() {
        let mut report = valid_refused_report();
        let object = report.as_object_mut().unwrap();
        object.remove("grant");
        object.remove("base_mission_digest");
        object.remove("totals");
        seal(&mut report);

        let verification = verify_autopilot_report(&report).unwrap();
        assert_eq!(verification["digest_match"], true);
        assert_eq!(verification["current_schema_shape_valid"], false);
        assert_eq!(verification["valid"], false);
        assert!(verification["shape_errors"].as_array().unwrap().len() >= 3);
    }

    #[test]
    fn current_report_rejects_a_restamped_but_invalid_embedded_grant() {
        let mut report = valid_refused_report();
        report["grant"]["allowed_tools"] = json!([]);
        report["grant_digest"] =
            json!(ContentHash::of_value(&report["grant"]).unwrap().to_string());
        seal(&mut report);

        let verification = verify_autopilot_report(&report).unwrap();
        assert_eq!(verification["digest_match"], true);
        assert_eq!(verification["current_schema_shape_valid"], false);
        assert_eq!(verification["valid"], false);
    }

    #[test]
    fn current_report_retains_an_invalid_response_as_distinct_from_transport_failure() {
        let mut report = valid_refused_report();
        report["attempts"][0]["outcome_summary"] = Value::Null;
        report["attempts"][0]["report_validation_error"] =
            json!("missing required mission_status field");
        report["attempts"][0]["classification_table"][0] = json!({
            "step_id": "measure",
            "status": "missing",
            "class": "unknown",
            "signal": "unrecognised_status",
            "reason": "the returned value was not a mission report",
        });
        report["final_status"] = json!("outcome_unknown");
        report
            .as_object_mut()
            .unwrap()
            .remove("first_terminal_refusal");
        report["outcome_unknown"] = json!({
            "reason": "invalid_mission_report",
            "detail": "the dispatch may already have caused side effects",
            "attempts_used": 1,
            "max_attempts": 1,
            "unresolved_steps": [{
                "step_id": "measure",
                "state": "unknown",
                "signal": "unrecognised_status",
                "reason": "the returned value was not a mission report",
                "attempt_index": 1,
                "exclusion": null,
            }],
        });
        seal(&mut report);

        let verification = verify_autopilot_report(&report).unwrap();
        assert_eq!(verification["current_schema_shape_valid"], true);
        assert_eq!(
            verification["reconciliation_digest_verification_supported"],
            true
        );
        assert_eq!(verification["valid"], true);

        report["outcome_unknown"]["unresolved_steps"][0]["step_id"] = json!("other-step");
        seal(&mut report);
        let verification = verify_autopilot_report(&report).unwrap();
        assert_eq!(verification["current_schema_shape_valid"], false);
        assert_eq!(verification["valid"], false);

        report["outcome_unknown"]["unresolved_steps"][0]["step_id"] = json!("measure");
        seal(&mut report);
        assert_eq!(verify_autopilot_report(&report).unwrap()["valid"], true);

        report["attempts"][0]["classification_table"][0]["class"] = json!("succeeded");
        seal(&mut report);
        let verification = verify_autopilot_report(&report).unwrap();
        assert_eq!(verification["current_schema_shape_valid"], false);
        assert_eq!(verification["valid"], false);
    }

    #[test]
    fn schema_0_6_remains_verifiable_without_reconciliation_records() {
        let mut report = valid_refused_report();
        report["schema"] = json!(PREVIOUS_AUTOPILOT_REPORT_SCHEMA_VERSION);
        report["limitations"] = json!(V0_6_REQUIRED_LIMITATIONS);
        report["attempts"][0]
            .as_object_mut()
            .unwrap()
            .remove("reconciliation_digest_verified");
        report["attempts"][0]
            .as_object_mut()
            .unwrap()
            .remove("reconciliation_record");
        seal(&mut report);

        assert_eq!(verify_autopilot_report(&report).unwrap()["valid"], true);
    }

    #[test]
    fn schema_0_5_keeps_its_unknown_outcome_contract() {
        let mut report = valid_refused_report();
        report["schema"] = json!(OLDER_AUTOPILOT_REPORT_SCHEMA_VERSION);
        report["limitations"] = json!(V0_5_REQUIRED_LIMITATIONS);
        report["final_status"] = json!("outcome_unknown");
        report
            .as_object_mut()
            .unwrap()
            .remove("first_terminal_refusal");
        report["outcome_unknown"] = json!({
            "reason": "dispatch_transport_error",
            "detail": "the report predates retained unknown-outcome step accounting",
        });
        seal(&mut report);

        let verification = verify_autopilot_report(&report).unwrap();
        assert_eq!(verification["final_status_known"], true);
        assert_eq!(verification["valid"], true);
        assert_eq!(
            verification["reconciliation_digest_verification_supported"],
            false
        );
    }

    #[test]
    fn current_report_recomputes_reconciliation_verification_from_its_embedded_record() {
        let mut report = valid_refused_report();
        let reconciliation = valid_reconciliation_record();
        report["attempts"][0]["reconciliation_record"] = reconciliation.clone();
        report["attempts"][0]["reconciliation_digest"] =
            reconciliation["reconciliation_digest"].clone();
        report["attempts"][0]["reconciliation_digest_verified"] = json!(true);
        report["attempts"][0]["reconciliation_status"] = json!({
            "completion": "complete",
            "integrity_valid": true,
        });
        report["attempts"][0]["reconciliation_scope"] = json!("full_plan");
        report["attempts"][0]["reconciliation_note"] = Value::Null;
        seal(&mut report);

        let verification = verify_autopilot_report(&report).unwrap();
        assert_eq!(verification["current_schema_shape_valid"], true);
        assert_eq!(
            verification["reconciliation_digest_verification_supported"],
            true
        );
        assert_eq!(verification["valid"], true);

        report["attempts"][0]["reconciliation_record"]["integrity"]["valid"] = json!(false);
        seal(&mut report);
        let verification = verify_autopilot_report(&report).unwrap();
        assert_eq!(verification["digest_match"], true);
        assert_eq!(verification["current_schema_shape_valid"], false);
        assert_eq!(verification["valid"], false);
    }

    #[test]
    fn schema_0_4_remains_verifiable_without_claiming_the_unknown_outcome_status() {
        let mut report = valid_refused_report();
        report["schema"] = json!(OLDEST_AUTOPILOT_REPORT_SCHEMA_VERSION);
        report["limitations"] = json!(V0_4_REQUIRED_LIMITATIONS);
        seal(&mut report);

        let verification = verify_autopilot_report(&report).unwrap();
        assert_eq!(
            verification["schema"],
            OLDEST_AUTOPILOT_REPORT_SCHEMA_VERSION
        );
        assert_eq!(verification["valid"], true);

        report["final_status"] = json!("outcome_unknown");
        report["outcome_unknown"] = json!({ "reason": "dispatch_transport_error" });
        seal(&mut report);
        let verification = verify_autopilot_report(&report).unwrap();
        assert_eq!(verification["final_status_known"], false);
        assert_eq!(verification["valid"], false);
    }

    #[test]
    fn current_report_cannot_claim_success_without_a_retained_attempt() {
        let mut report = valid_refused_report();
        report["final_status"] = json!("succeeded");
        report
            .as_object_mut()
            .unwrap()
            .remove("first_terminal_refusal");
        report["attempts"] = json!([]);
        report["totals"]["attempts_used"] = json!(0);
        report["evidence"] = json!({
            "mission_status": "succeeded",
            "steps": [{
                "step_id": "measure",
                "attempt_index": 1,
                "status": "succeeded",
                "result_digest": null,
                "arguments_digest": null,
            }],
            "reconciliation": {
                "required": false,
                "note": "the grant waived the reconciliation-complete requirement",
            },
        });
        seal(&mut report);

        let verification = verify_autopilot_report(&report).unwrap();
        assert_eq!(verification["final_detail_valid"], true);
        assert_eq!(verification["digest_match"], true);
        assert_eq!(verification["current_schema_shape_valid"], false);
        assert_eq!(verification["valid"], false);
    }

    fn valid_refused_report() -> Value {
        let grant = json!({
            "allowed_tools": ["measure"],
            "allow_side_effects": false,
            "max_attempts": 1,
            "retry": {
                "retry_retryable_as_is": true,
                "retry_retryable_after_change": false,
                "retry_unknown": false,
            },
            "schedule": { "retry_base_delay": 0, "retry_max_delay": 0 },
            "require_reconciliation_complete": false,
            "stop_on_first_success": true,
        });
        let grant_digest = ContentHash::of_value(&grant).unwrap().to_string();
        json!({
            "schema": AUTOPILOT_REPORT_SCHEMA_VERSION,
            "grant_digest": grant_digest,
            "grant": grant,
            "base_mission_id": "report-shape-test",
            "base_mission_digest": "a".repeat(64),
            "attempts": [{
                "attempt_index": 1,
                "kind": "full",
                "mission_digest": "b".repeat(64),
                "dispatched_step_ids": ["measure"],
                "report_digest": "c".repeat(64),
                "outcome_summary": {
                    "mission_status": "failed",
                    "succeeded": 0,
                    "refused": 1,
                    "blocked": 0,
                    "cancelled": 0,
                    "required_failures": 1,
                },
                "classification_table": [{
                    "step_id": "measure",
                    "status": "refused",
                    "class": "terminal",
                    "signal": "executor_refusal",
                    "reason": "the executor refused the step",
                }],
                "report_validation_error": null,
                "reconciliation_record": null,
                "reconciliation_digest": null,
                "reconciliation_digest_verified": null,
                "reconciliation_status": null,
                "reconciliation_scope": null,
                "reconciliation_note": "no reconciliation was available",
                "dispatch_error": null,
            }],
            "final_status": "refused",
            "totals": {
                "attempts_used": 1,
                "max_attempts": 1,
                "steps_in_plan": 1,
            },
            "limitations": REQUIRED_LIMITATIONS,
            "first_terminal_refusal": {
                "step_id": "measure",
                "tool": "measure",
                "attempt_index": 1,
                "status": "refused",
                "signal": "executor_refusal",
                "reason": "the executor refused the step",
                "error": null,
            },
        })
    }

    fn valid_reconciliation_record() -> Value {
        let mut record = json!({
            "ok": true,
            "workflow": "domain_workflow_reconcile",
            "schema": bioprism_devplat::DOMAIN_WORKFLOW_RECONCILE_SCHEMA_VERSION,
            "execution": "not_started",
            "source": "mission_report",
            "workflow_id": "workflow-a",
            "mission_id": "mission-a",
            "workflow_digest": "a".repeat(64),
            "catalog_digest": "b".repeat(64),
            "domain_contract_digest": "c".repeat(64),
            "mission_plan_digest": "d".repeat(64),
            "completion": {
                "status": "complete",
                "ready": true,
                "review_required": true,
                "claims_posture": "review_required_before_claims",
            },
            "evidence": { "evidence_valid": true },
            "integrity": { "valid": true, "finding_count": 0, "findings": [] },
        });
        record["reconciliation_digest"] =
            json!(ContentHash::of_value(&record).unwrap().to_string());
        record
    }

    fn seal(report: &mut Value) {
        report
            .as_object_mut()
            .expect("report is an object")
            .remove("report_sha256");
        let digest = ContentHash::of_value(report).unwrap().to_string();
        report["report_sha256"] = Value::String(digest);
    }
}
