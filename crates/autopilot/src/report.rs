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
//! report that omitted them would imply capabilities — scheduling, MCP exposure, unrestricted
//! resume, deadlines — that this crate deliberately does not have.
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
use crate::history::DriveHistory;
use bioprism_ids::ContentHash;
use serde_json::{json, Value};

pub const AUTOPILOT_REPORT_SCHEMA_VERSION: &str = "bioprism-autopilot/report/0.1";

/// The limitations every report must carry. Verification refuses a report missing any of them.
pub const REQUIRED_LIMITATIONS: [&str; 4] = [
    "no recurrence: the drive runs one mission to a stop state and never repeats a completed mission",
    "no MCP tool exposure: the autopilot is not an MCP tool and registers nothing with the server",
    "metadata-only cross-process resume: checkpoints retain digests and bounded status metadata, while callers must rehydrate private mission and report material",
    "wall-clock ownership and deadlines remain caller-owned: a grant may authorize logical-tick retry backoff, but the wait seam and deadline policy live outside the kernel",
];

const ADDITIONAL_LIMITATIONS: [&str; 4] = [
    "an undelivered dispatch is never re-sent: a missing mission report leaves side effects unknown at mission level",
    "a repair attempt's reconciliation covers only the re-dispatched subset and is labelled with that scope",
    "succeeded steps are never re-dispatched; a binding whose retained payload is gone excludes its dependent instead",
    "a repair re-dispatches steps without the base mission's claim requests or reviews; claim lineage exists only for attempt 1",
];

/// How a drive ended. The three values are the only final statuses a report may carry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FinalStatus {
    Succeeded,
    Exhausted,
    Refused,
}

impl FinalStatus {
    pub const ALL: [FinalStatus; 3] = [
        FinalStatus::Succeeded,
        FinalStatus::Exhausted,
        FinalStatus::Refused,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            FinalStatus::Succeeded => "succeeded",
            FinalStatus::Exhausted => "exhausted",
            FinalStatus::Refused => "refused",
        }
    }
}

/// The stop the planner returned, carried into the report with its structured detail.
#[derive(Debug, Clone, PartialEq)]
pub enum FinalDisposition {
    Succeeded { evidence: Value },
    Exhausted { accounting: Value },
    Refused { first_terminal_refusal: Value },
}

impl FinalDisposition {
    pub fn status(&self) -> FinalStatus {
        match self {
            FinalDisposition::Succeeded { .. } => FinalStatus::Succeeded,
            FinalDisposition::Exhausted { .. } => FinalStatus::Exhausted,
            FinalDisposition::Refused { .. } => FinalStatus::Refused,
        }
    }

    fn detail(&self) -> (&'static str, &Value) {
        match self {
            FinalDisposition::Succeeded { evidence } => ("evidence", evidence),
            FinalDisposition::Exhausted { accounting } => ("accounting", accounting),
            FinalDisposition::Refused {
                first_terminal_refusal,
            } => ("first_terminal_refusal", first_terminal_refusal),
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
                .parsed_report()
                .map(|_| {
                    attempt
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
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
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
            let (reconciliation_digest, reconciliation_status, reconciliation_scope) =
                match attempt.reconciliation_summary() {
                    Some((status, integrity_valid, digest)) => (
                        digest.map(Value::String).unwrap_or(Value::Null),
                        json!({ "completion": status, "integrity_valid": integrity_valid }),
                        Value::String(attempt.kind().reconciliation_scope().into()),
                    ),
                    None => (Value::Null, Value::Null, Value::Null),
                };
            json!({
                "attempt_index": index + 1,
                "kind": attempt.kind().as_str(),
                "mission_digest": attempt.mission_digest(),
                "dispatched_step_ids": attempt.dispatched_step_ids(),
                "report_digest": attempt.report_digest(),
                "outcome_summary": outcome_summary,
                "classification_table": classification_table,
                "reconciliation_digest": reconciliation_digest,
                "reconciliation_status": reconciliation_status,
                "reconciliation_scope": reconciliation_scope,
                "reconciliation_note": attempt.reconciliation_note(),
                "dispatch_error": attempt.dispatch_error(),
            })
        })
        .collect()
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
    if schema != AUTOPILOT_REPORT_SCHEMA_VERSION {
        return Err(AutopilotError::InvalidAutopilotReport {
            reason: format!("schema is {schema:?}, expected {AUTOPILOT_REPORT_SCHEMA_VERSION:?}"),
        });
    }
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
    let missing_limitations = REQUIRED_LIMITATIONS
        .iter()
        .filter(|required| !limitation_texts.iter().any(|text| text == *required))
        .map(|required| Value::String((*required).into()))
        .collect::<Vec<_>>();
    let limitations_present = !limitation_texts.is_empty() && missing_limitations.is_empty();

    let final_status = object
        .get("final_status")
        .and_then(Value::as_str)
        .unwrap_or("");
    let final_status_known = FinalStatus::ALL
        .iter()
        .any(|status| status.as_str() == final_status);
    let attempts_present = object.get("attempts").is_some_and(Value::is_array);

    let valid = !digest_malformed
        && digest_match
        && limitations_present
        && final_status_known
        && attempts_present;
    Ok(json!({
        "schema": AUTOPILOT_REPORT_SCHEMA_VERSION,
        "valid": valid,
        "digest_malformed": digest_malformed,
        "digest_match": digest_match,
        "claimed_report_sha256": claimed,
        "recomputed_report_sha256": recomputed,
        "limitations_present": limitations_present,
        "missing_limitations": missing_limitations,
        "final_status_known": final_status_known,
        "attempts_present": attempts_present,
    }))
}
