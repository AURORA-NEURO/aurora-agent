//! Structural provenance reconciliation for executed missions and delegated checks.
//!
//! A mission report already contains deterministic events, but callers often need one bounded
//! artifact that answers a narrower question: does the returned report account for every planned
//! step, preserve event ordering and tool identity, and keep delegated check outcomes attached to
//! the same execution? This module performs that reconciliation without re-running the mission,
//! calling a provider, or turning caller-supplied evidence into an execution claim.

use std::collections::{BTreeMap, BTreeSet};

use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::mission::{
    MissionReport, MissionTraceEvent, MISSION_SCHEMA_VERSION, MISSION_TRACE_SCHEMA_VERSION,
};

pub const EXECUTION_PROVENANCE_SCHEMA: &str = "bioprism-devplat-execution-provenance/0.1";
pub const MAX_DELEGATED_CHECKS: usize = 64;
pub const MAX_FINDINGS: usize = 128;
const MAX_TRACE_EVENTS: usize = 4_096;
const MAX_TEXT_BYTES: usize = 4_096;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DelegatedCheckEvidence {
    pub name: String,
    pub kind: String,
    pub required: bool,
    pub status: String,
    pub result_digest: String,
    pub source: String,
    #[serde(default)]
    pub trace_sequence: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ExecutionProvenanceRequest {
    pub mission: MissionReport,
    #[serde(default)]
    pub delegated_checks: Vec<DelegatedCheckEvidence>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ExecutionProvenanceFinding {
    pub code: String,
    pub severity: String,
    pub subject: String,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ExecutionProvenanceAudit {
    pub schema: String,
    pub mission_id: String,
    pub plan_digest: String,
    pub trace_digest: String,
    pub provenance_digest: String,
    pub mission_execution: String,
    pub mission_status: String,
    pub planned_step_count: usize,
    pub result_count: usize,
    pub trace_event_count: usize,
    pub delegated_check_count: usize,
    pub succeeded_step_count: usize,
    pub refused_step_count: usize,
    pub blocked_step_count: usize,
    pub cancelled_step_count: usize,
    pub required_failure_count: usize,
    pub required_check_count: usize,
    pub passed_check_count: usize,
    pub nonpassing_required_checks: Vec<String>,
    pub missing_step_results: Vec<String>,
    pub unknown_step_results: Vec<String>,
    pub duplicate_trace_sequences: Vec<usize>,
    pub trace_identity_errors: Vec<String>,
    pub complete: bool,
    pub structurally_valid: bool,
    pub release_candidate: bool,
    pub execution: String,
    pub verification: String,
    pub findings: Vec<ExecutionProvenanceFinding>,
    pub guarantees: Vec<String>,
    pub limitations: Vec<String>,
}

fn valid_digest(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        && ContentHash::parse(value.to_owned()).is_ok()
}

fn valid_text(value: &str) -> bool {
    let trimmed = value.trim();
    !trimmed.is_empty()
        && value == trimmed
        && value.len() <= MAX_TEXT_BYTES
        && !value.chars().any(char::is_control)
}

fn valid_identifier(value: &str) -> bool {
    valid_text(value) && value == value.trim()
}

fn finding(
    findings: &mut Vec<ExecutionProvenanceFinding>,
    code: &str,
    severity: &str,
    subject: impl Into<String>,
    detail: impl Into<String>,
) {
    if findings.len() >= MAX_FINDINGS {
        if findings.iter().any(|item| item.code == "finding_overflow") {
            return;
        }
        findings.truncate(MAX_FINDINGS - 1);
        findings.push(ExecutionProvenanceFinding {
            code: "finding_overflow".into(),
            severity: "blocking".into(),
            subject: "findings".into(),
            detail: format!(
                "more than {MAX_FINDINGS} findings were generated; the audit is incomplete"
            ),
        });
        return;
    }
    findings.push(ExecutionProvenanceFinding {
        code: code.into(),
        severity: severity.into(),
        subject: subject.into(),
        detail: detail.into(),
    });
}

fn terminal_event(event: &str) -> bool {
    matches!(
        event,
        "step.completed" | "step.refused" | "step.blocked" | "step.cancelled"
    )
}

fn terminal_event_matches_result(event: &str, status: &str) -> bool {
    match event {
        "step.completed" => matches!(status, "succeeded" | "completed"),
        "step.refused" => status == "refused",
        "step.blocked" => status == "blocked",
        "step.cancelled" => status == "cancelled",
        _ => true,
    }
}

fn supported_trace_event(event: &str) -> bool {
    matches!(
        event,
        "mission.started"
            | "mission.cancelled"
            | "mission.completed"
            | "wave.started"
            | "wave.completed"
            | "step.started"
            | "step.completed"
            | "step.refused"
            | "step.blocked"
            | "step.cancelled"
    )
}

fn event_step(event: &MissionTraceEvent) -> Option<&str> {
    event.step_id.as_deref()
}

fn digest_value(value: &serde_json::Value, label: &str) -> Result<String, String> {
    ContentHash::of_value(value)
        .map(|digest| digest.to_string())
        .map_err(|error| format!("cannot hash {label}: {error}"))
}

/// Reconcile one mission report and its delegated evidence without executing anything.
pub fn audit_execution_provenance(
    request: &ExecutionProvenanceRequest,
) -> Result<ExecutionProvenanceAudit, String> {
    let mission = &request.mission;
    let plan = &mission.plan;
    let mut findings = Vec::new();

    if mission.schema_version != MISSION_SCHEMA_VERSION {
        finding(
            &mut findings,
            "mission_schema_invalid",
            "blocking",
            "mission",
            "mission report schema version is not supported",
        );
    }
    if plan.schema_version != MISSION_SCHEMA_VERSION {
        finding(
            &mut findings,
            "plan_schema_invalid",
            "blocking",
            "plan",
            "mission plan schema version is not supported",
        );
    }
    if mission.execution_trace_schema_version != MISSION_TRACE_SCHEMA_VERSION {
        finding(
            &mut findings,
            "trace_schema_invalid",
            "blocking",
            "execution_trace",
            "execution trace schema version is not supported",
        );
    }

    if plan.mission_id.trim().is_empty() {
        finding(
            &mut findings,
            "mission_id_missing",
            "blocking",
            "mission",
            "mission and plan identifiers are required",
        );
    } else if !valid_identifier(&plan.mission_id) {
        finding(
            &mut findings,
            "mission_id_invalid",
            "blocking",
            "mission",
            "mission and plan identifiers must be bounded visible identifiers",
        );
    }
    if !valid_digest(&plan.digest) {
        finding(
            &mut findings,
            "plan_digest_invalid",
            "blocking",
            "plan",
            "plan digest must be a 64-character hexadecimal content digest",
        );
    }
    if !valid_identifier(&mission.execution) {
        finding(
            &mut findings,
            "mission_execution_invalid",
            "blocking",
            "mission",
            "mission execution state must be bounded visible text",
        );
    }
    if !matches!(plan.execution.as_str(), "authorized" | "planned") {
        finding(
            &mut findings,
            "plan_execution_invalid",
            "blocking",
            "plan",
            "mission plan execution posture must be authorized or planned",
        );
    }
    if matches!(mission.execution.as_str(), "executed" | "planned") {
        let expected = if mission.execution == "executed" {
            "authorized"
        } else {
            "planned"
        };
        if plan.execution != expected {
            finding(
                &mut findings,
                "plan_execution_mismatch",
                "blocking",
                "plan",
                format!(
                    "mission execution `{}` requires plan execution posture `{expected}`",
                    mission.execution
                ),
            );
        }
    }
    if !matches!(plan.execution_mode.as_str(), "serial" | "parallel_waves")
        || plan.max_parallelism == 0
    {
        finding(
            &mut findings,
            "plan_execution_mode_invalid",
            "blocking",
            "plan",
            "mission plan execution mode and parallelism must be bounded supported values",
        );
    }
    if !valid_identifier(&mission.mission_status) {
        finding(
            &mut findings,
            "mission_status_invalid",
            "blocking",
            "mission",
            "mission status must be bounded visible text",
        );
    }
    if mission.execution != "executed" {
        finding(
            &mut findings,
            "mission_not_executed",
            "warning",
            "mission",
            "the supplied report is not marked as an executed mission",
        );
    }
    if mission.execution_trace.len() > MAX_TRACE_EVENTS {
        return Err(format!(
            "mission execution_trace exceeds the {MAX_TRACE_EVENTS}-event safety bound"
        ));
    }
    if mission.results.len() > plan.steps.len().saturating_add(64) {
        finding(
            &mut findings,
            "result_overflow",
            "blocking",
            "results",
            "mission returned more step results than its bounded plan permits",
        );
    }
    if request.delegated_checks.len() > MAX_DELEGATED_CHECKS {
        return Err(format!(
            "delegated_checks exceeds the {MAX_DELEGATED_CHECKS}-check safety bound"
        ));
    }

    if plan.steps.is_empty() {
        finding(
            &mut findings,
            "plan_steps_missing",
            "blocking",
            "plan",
            "an executable provenance report must contain at least one planned step",
        );
    }
    if plan.step_count != plan.steps.len() {
        finding(
            &mut findings,
            "plan_step_count_mismatch",
            "blocking",
            "plan",
            format!(
                "plan declares {} steps but embeds {}",
                plan.step_count,
                plan.steps.len()
            ),
        );
    }
    let mut planned = BTreeMap::new();
    let mut planned_ids = BTreeSet::new();
    for step in &plan.steps {
        if !valid_identifier(&step.id) || !valid_text(&step.tool) {
            finding(
                &mut findings,
                "planned_step_identity_invalid",
                "blocking",
                step.id.clone(),
                "planned step id and tool must be bounded visible metadata",
            );
        }
        if !planned_ids.insert(step.id.to_ascii_lowercase()) {
            finding(
                &mut findings,
                "duplicate_planned_step",
                "blocking",
                step.id.clone(),
                "planned step identifiers must be unique without case-folding collisions",
            );
        }
        planned.insert(step.id.as_str(), (step.tool.as_str(), step.required));
    }
    let ordered_step_ids = plan
        .steps
        .iter()
        .map(|step| step.id.clone())
        .collect::<Vec<_>>();
    if plan.ordered_steps != ordered_step_ids {
        finding(
            &mut findings,
            "plan_order_mismatch",
            "blocking",
            "plan",
            "ordered_steps must match the embedded mission step order",
        );
    }
    let wave_step_ids = plan.waves.iter().flatten().cloned().collect::<Vec<_>>();
    if plan.ordered_steps != wave_step_ids {
        finding(
            &mut findings,
            "plan_wave_mismatch",
            "blocking",
            "plan",
            "flattened mission waves must match ordered_steps",
        );
    }
    if plan.critical_path_length != plan.waves.len() {
        finding(
            &mut findings,
            "plan_wave_count_mismatch",
            "blocking",
            "plan",
            "critical_path_length must match the number of mission waves",
        );
    }
    let mut results = BTreeMap::new();
    let mut succeeded_step_count = 0;
    let mut refused_step_count = 0;
    let mut blocked_step_count = 0;
    let mut cancelled_step_count = 0;
    let mut required_failure_count = 0;
    let mut result_ids = BTreeSet::new();
    for result in &mission.results {
        if !valid_identifier(&result.id) || !valid_text(&result.tool) {
            finding(
                &mut findings,
                "step_result_identity_invalid",
                "blocking",
                result.id.clone(),
                "step result id and tool must be bounded visible metadata",
            );
        }
        if !result_ids.insert(result.id.to_ascii_lowercase()) {
            finding(
                &mut findings,
                "duplicate_step_result",
                "blocking",
                result.id.clone(),
                "step result identifiers must be unique without case-folding collisions",
            );
        }
        if results.insert(result.id.as_str(), result).is_some() {
            finding(
                &mut findings,
                "duplicate_step_result",
                "blocking",
                result.id.clone(),
                "a mission step has more than one returned result",
            );
        }
        let Some((tool, required)) = planned.get(result.id.as_str()) else {
            finding(
                &mut findings,
                "unknown_step_result",
                "blocking",
                result.id.clone(),
                "result does not identify a step in the embedded plan",
            );
            continue;
        };
        if result.tool != *tool {
            finding(
                &mut findings,
                "step_tool_mismatch",
                "blocking",
                result.id.clone(),
                "step result tool does not match the planned tool",
            );
        }
        if result.required != *required {
            finding(
                &mut findings,
                "step_required_mismatch",
                "blocking",
                result.id.clone(),
                "step result required flag does not match the planned step",
            );
        }
        if !valid_identifier(&result.status) {
            finding(
                &mut findings,
                "step_result_status_invalid",
                "blocking",
                result.id.clone(),
                "step result status must be bounded visible text",
            );
        }
        if result
            .arguments_digest
            .as_deref()
            .is_some_and(|digest| !valid_digest(digest))
        {
            finding(
                &mut findings,
                "step_result_digest_invalid",
                "blocking",
                result.id.clone(),
                "step result arguments_digest must be a canonical content digest",
            );
        }
        if result
            .error
            .as_deref()
            .is_some_and(|error| !valid_text(error))
        {
            finding(
                &mut findings,
                "step_result_error_invalid",
                "blocking",
                result.id.clone(),
                "step result error text must be bounded visible text",
            );
        }
        match result.status.as_str() {
            "succeeded" | "completed" => succeeded_step_count += 1,
            "refused" => {
                refused_step_count += 1;
                if *required {
                    required_failure_count += 1;
                }
            }
            "blocked" => {
                blocked_step_count += 1;
                if *required {
                    required_failure_count += 1;
                }
            }
            "cancelled" => {
                cancelled_step_count += 1;
                if *required {
                    required_failure_count += 1;
                }
            }
            other => finding(
                &mut findings,
                "unknown_step_status",
                "blocking",
                result.id.clone(),
                format!("unsupported mission result status: {other}"),
            ),
        }
    }

    for (label, declared, observed) in [
        ("succeeded", mission.succeeded, succeeded_step_count),
        ("refused", mission.refused, refused_step_count),
        ("blocked", mission.blocked, blocked_step_count),
        ("cancelled", mission.cancelled, cancelled_step_count),
        (
            "required_failures",
            mission.required_failures,
            required_failure_count,
        ),
    ] {
        if declared != observed {
            finding(
                &mut findings,
                "mission_counter_mismatch",
                "blocking",
                label,
                format!("mission declares {declared} {label} outcomes but reconciled {observed}"),
            );
        }
    }

    let missing_step_results = planned
        .keys()
        .filter(|id| !results.contains_key(**id))
        .map(|id| (*id).to_string())
        .collect::<Vec<_>>();
    for id in &missing_step_results {
        finding(
            &mut findings,
            "missing_step_result",
            "blocking",
            id,
            "every planned step requires one terminal result in the provenance report",
        );
    }
    let unknown_step_results = mission
        .results
        .iter()
        .filter(|result| !planned.contains_key(result.id.as_str()))
        .map(|result| result.id.clone())
        .collect::<Vec<_>>();

    let mut sequences = BTreeSet::new();
    let mut duplicate_trace_sequences = Vec::new();
    let mut trace_identity_errors = Vec::new();
    let mut terminal_steps = BTreeSet::new();
    for (index, event) in mission.execution_trace.iter().enumerate() {
        if !supported_trace_event(&event.event) {
            trace_identity_errors.push(format!(
                "trace contains unsupported event type at index {index}: {}",
                event.event
            ));
        }
        if !valid_text(&event.event)
            || event
                .step_id
                .as_deref()
                .is_some_and(|step_id| !valid_identifier(step_id))
            || event.tool.as_deref().is_some_and(|tool| !valid_text(tool))
            || event
                .status
                .as_deref()
                .is_some_and(|status| !valid_text(status))
            || event
                .detail
                .as_deref()
                .is_some_and(|detail| !valid_text(detail))
            || event
                .arguments_digest
                .as_deref()
                .is_some_and(|digest| !valid_digest(digest))
        {
            trace_identity_errors.push(format!(
                "trace event at index {index} contains invalid bounded identity metadata"
            ));
        }
        if event.sequence != index {
            trace_identity_errors.push(format!(
                "trace event at index {index} declares sequence {}",
                event.sequence
            ));
        }
        if !sequences.insert(event.sequence) {
            duplicate_trace_sequences.push(event.sequence);
        }
        if let Some(step_id) = event_step(event) {
            let Some((tool, _)) = planned.get(step_id) else {
                trace_identity_errors.push(format!("trace references unknown step: {step_id}"));
                continue;
            };
            if event.tool.as_deref() != Some(*tool) {
                trace_identity_errors.push(format!("trace tool mismatch for step: {step_id}"));
            }
            if terminal_event(&event.event) {
                if let Some(result) = results.get(step_id) {
                    if !terminal_event_matches_result(&event.event, result.status.as_str()) {
                        trace_identity_errors.push(format!(
                            "terminal trace event {} does not match result status {} for step: {step_id}",
                            event.event, result.status
                        ));
                    }
                    if event
                        .status
                        .as_deref()
                        .is_none_or(|status| !terminal_event_matches_result(&event.event, status))
                    {
                        trace_identity_errors.push(format!(
                            "terminal trace event status does not match event {} for step: {step_id}",
                            event.event
                        ));
                    }
                }
            }
            if terminal_event(&event.event) && !terminal_steps.insert(step_id.to_string()) {
                trace_identity_errors
                    .push(format!("step has duplicate terminal events: {step_id}"));
            }
        } else if event.event.starts_with("step.") {
            trace_identity_errors.push(format!("step event has no step_id: {}", event.event));
        }
    }
    for error in &trace_identity_errors {
        finding(
            &mut findings,
            "trace_identity_error",
            "blocking",
            "execution_trace",
            error,
        );
    }
    for sequence in &duplicate_trace_sequences {
        finding(
            &mut findings,
            "duplicate_trace_sequence",
            "blocking",
            sequence.to_string(),
            "trace sequence numbers must be unique",
        );
    }
    if mission
        .execution_trace
        .first()
        .map(|event| event.event.as_str())
        != Some("mission.started")
    {
        finding(
            &mut findings,
            "trace_start_missing",
            "blocking",
            "execution_trace",
            "a mission trace must begin with mission.started",
        );
    }
    if mission
        .execution_trace
        .last()
        .map(|event| event.event.as_str())
        != Some("mission.completed")
    {
        finding(
            &mut findings,
            "trace_completion_missing",
            "blocking",
            "execution_trace",
            "a mission trace must end with mission.completed",
        );
    }
    for id in planned.keys() {
        if !terminal_steps.contains(*id) {
            finding(
                &mut findings,
                "missing_terminal_trace",
                "blocking",
                *id,
                "every planned step requires one terminal trace event",
            );
        }
    }

    let mut delegated_names = BTreeSet::new();
    let mut required_check_count = 0;
    let mut passed_check_count = 0;
    let mut nonpassing_required_checks = Vec::new();
    for check in &request.delegated_checks {
        if !valid_identifier(&check.name)
            || !valid_identifier(&check.kind)
            || !valid_text(&check.source)
            || !valid_identifier(&check.status)
        {
            finding(
                &mut findings,
                "delegated_check_identity_missing",
                "blocking",
                "delegated_checks",
                "name, kind, and source are required",
            );
        }
        if !delegated_names.insert(check.name.to_ascii_lowercase()) {
            finding(
                &mut findings,
                "duplicate_delegated_check",
                "blocking",
                check.name.clone(),
                "delegated check names must be unique",
            );
        }
        if !valid_digest(&check.result_digest) {
            finding(
                &mut findings,
                "delegated_check_digest_invalid",
                "blocking",
                check.name.clone(),
                "delegated check result_digest must be a 64-character hexadecimal digest",
            );
        }
        if let Some(sequence) = check.trace_sequence {
            if !sequences.contains(&sequence) {
                finding(
                    &mut findings,
                    "delegated_trace_reference_missing",
                    "blocking",
                    check.name.clone(),
                    "delegated check references a trace sequence absent from the mission trace",
                );
            }
        }
        if check.required {
            required_check_count += 1;
            if check.status == "passed" {
                passed_check_count += 1;
            } else {
                nonpassing_required_checks.push(check.name.clone());
            }
        } else if check.status == "passed" {
            passed_check_count += 1;
        }
        if !matches!(
            check.status.as_str(),
            "passed" | "failed" | "refused" | "not_run" | "unknown"
        ) {
            finding(
                &mut findings,
                "unknown_delegated_check_status",
                "blocking",
                check.name.clone(),
                format!("unsupported delegated check status: {}", check.status),
            );
        }
    }
    for name in &nonpassing_required_checks {
        finding(
            &mut findings,
            "required_delegated_check_not_passing",
            "blocking",
            name,
            "required delegated checks must report passed before a release candidate can be formed",
        );
    }

    let trace_digest = digest_value(
        &serde_json::to_value(&mission.execution_trace)
            .map_err(|error| format!("cannot encode execution trace: {error}"))?,
        "execution trace",
    )?;
    let provenance_input = json!({
        "mission_id": plan.mission_id,
        "plan_digest": plan.digest,
        "trace_digest": trace_digest,
        "results": mission.results,
        "delegated_checks": request.delegated_checks,
    });
    let provenance_digest = digest_value(&provenance_input, "execution provenance")?;
    let structurally_valid = findings.iter().all(|item| item.severity != "blocking");
    let complete = missing_step_results.is_empty()
        && terminal_steps.len() == planned.len()
        && mission.execution_trace.len() >= 2;
    let release_candidate = structurally_valid
        && complete
        && mission.execution == "executed"
        && mission.mission_status == "succeeded"
        && required_failure_count == 0
        && nonpassing_required_checks.is_empty();

    Ok(ExecutionProvenanceAudit {
        schema: EXECUTION_PROVENANCE_SCHEMA.into(),
        mission_id: plan.mission_id.clone(),
        plan_digest: plan.digest.clone(),
        trace_digest,
        provenance_digest,
        mission_execution: mission.execution.clone(),
        mission_status: mission.mission_status.clone(),
        planned_step_count: plan.steps.len(),
        result_count: mission.results.len(),
        trace_event_count: mission.execution_trace.len(),
        delegated_check_count: request.delegated_checks.len(),
        succeeded_step_count,
        refused_step_count,
        blocked_step_count,
        cancelled_step_count,
        required_failure_count,
        required_check_count,
        passed_check_count,
        nonpassing_required_checks,
        missing_step_results,
        unknown_step_results,
        duplicate_trace_sequences,
        trace_identity_errors,
        complete,
        structurally_valid,
        release_candidate,
        execution: "evidence_supplied_not_executed_here".into(),
        verification: "structural_only".into(),
        findings,
        guarantees: vec![
            "plan, results, trace events, and delegated check identities are reconciled in one bounded report".into(),
            "content digests bind the returned trace and provenance projection without trusting a caller-asserted provenance digest".into(),
            "release_candidate requires executed/succeeded mission evidence and passing required delegated checks".into(),
        ],
        limitations: vec![
            "this route does not execute or replay the mission".into(),
            "delegated result digests are caller/provider evidence and are not fetched or cryptographically attested here".into(),
            "structural validity is not deployment, security, scientific, clinical, or provider approval".into(),
        ],
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mission::{MissionPlan, MissionStepPlan, MissionStepResult};

    fn report() -> MissionReport {
        let plan = MissionPlan {
            schema_version: "bioprism-devplat-mission/0.1".into(),
            mission_id: "mission-provenance".into(),
            goal: "audit provenance".into(),
            digest: "a".repeat(64),
            step_count: 1,
            ordered_steps: vec!["one".into()],
            waves: vec![vec!["one".into()]],
            critical_path_length: 1,
            steps: vec![MissionStepPlan {
                id: "one".into(),
                domain: "workspace".into(),
                capability: "audit".into(),
                objective: "audit".into(),
                tool: "echo".into(),
                depends_on: vec![],
                bindings: vec![],
                required: true,
                wave: 0,
            }],
            execution: "authorized".into(),
            execution_mode: "serial".into(),
            max_parallelism: 1,
            workflow_binding: None,
            route_review_provenance: None,
            guarantees: vec![],
            limitations: vec![],
        };
        MissionReport {
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
            results: vec![MissionStepResult {
                id: "one".into(),
                tool: "echo".into(),
                status: "succeeded".into(),
                required: true,
                arguments_digest: Some("b".repeat(64)),
                bytes: 2,
                wire: Some(json!({"ok": true})),
                error: None,
            }],
            execution_trace_schema_version: "bioprism-devplat-mission-trace/0.1".into(),
            execution_trace: vec![
                MissionTraceEvent {
                    sequence: 0,
                    event: "mission.started".into(),
                    wave: None,
                    step_id: None,
                    tool: None,
                    status: Some("running".into()),
                    arguments_digest: None,
                    bytes: 0,
                    detail: None,
                },
                MissionTraceEvent {
                    sequence: 1,
                    event: "step.completed".into(),
                    wave: Some(0),
                    step_id: Some("one".into()),
                    tool: Some("echo".into()),
                    status: Some("succeeded".into()),
                    arguments_digest: Some("b".repeat(64)),
                    bytes: 2,
                    detail: None,
                },
                MissionTraceEvent {
                    sequence: 2,
                    event: "mission.completed".into(),
                    wave: None,
                    step_id: None,
                    tool: None,
                    status: Some("succeeded".into()),
                    arguments_digest: None,
                    bytes: 2,
                    detail: None,
                },
            ],
            claim_requests: vec![],
            evaluator_review: None,
            claim_lineage: json!({}),
            trace_observer: None,
            guarantees: vec![],
            limitations: vec![],
        }
    }

    #[test]
    fn complete_passing_mission_becomes_a_structural_release_candidate() {
        let audit = audit_execution_provenance(&ExecutionProvenanceRequest {
            mission: report(),
            delegated_checks: vec![DelegatedCheckEvidence {
                name: "unit_tests".into(),
                kind: "test".into(),
                required: true,
                status: "passed".into(),
                result_digest: "c".repeat(64),
                source: "caller_attested".into(),
                trace_sequence: Some(1),
            }],
        })
        .unwrap();
        assert!(audit.complete);
        assert!(audit.structurally_valid);
        assert!(audit.release_candidate);
        assert_eq!(audit.passed_check_count, 1);
    }

    #[test]
    fn missing_trace_and_required_check_fail_closed() {
        let mut mission = report();
        mission.execution_trace.pop();
        mission.results[0].status = "refused".into();
        let audit = audit_execution_provenance(&ExecutionProvenanceRequest {
            mission,
            delegated_checks: vec![DelegatedCheckEvidence {
                name: "unit_tests".into(),
                kind: "test".into(),
                required: true,
                status: "failed".into(),
                result_digest: "d".repeat(64),
                source: "provider_observed".into(),
                trace_sequence: None,
            }],
        })
        .unwrap();
        assert!(!audit.structurally_valid);
        assert!(!audit.release_candidate);
        assert!(audit
            .findings
            .iter()
            .any(|finding| finding.code == "trace_completion_missing"));
        assert_eq!(audit.nonpassing_required_checks, vec!["unit_tests"]);
    }

    #[test]
    fn provenance_reconciles_declared_counters_and_canonical_evidence() {
        let mut mission = report();
        mission.succeeded = 2;
        mission.execution_trace[1].tool = Some("echo\u{0000}".into());
        let audit = audit_execution_provenance(&ExecutionProvenanceRequest {
            mission,
            delegated_checks: vec![DelegatedCheckEvidence {
                name: "unit_tests".into(),
                kind: "test".into(),
                required: true,
                status: "passed".into(),
                result_digest: "A".repeat(64),
                source: "caller_attested".into(),
                trace_sequence: Some(1),
            }],
        })
        .expect("audit");
        assert!(!audit.structurally_valid);
        for code in [
            "mission_counter_mismatch",
            "trace_identity_error",
            "delegated_check_digest_invalid",
        ] {
            assert!(
                audit.findings.iter().any(|finding| finding.code == code),
                "missing {code}"
            );
        }
    }

    #[test]
    fn provenance_rejects_padded_and_noncanonical_nested_metadata() {
        let mut mission = report();
        mission.results[0].status = " succeeded".into();
        mission.results[0].error = Some(" caller error".into());
        mission.execution_trace[1].status = Some(" succeeded".into());
        mission.execution_trace[1].arguments_digest = Some("A".repeat(64));
        let audit = audit_execution_provenance(&ExecutionProvenanceRequest {
            mission,
            delegated_checks: vec![DelegatedCheckEvidence {
                name: "unit_tests".into(),
                kind: "test".into(),
                required: true,
                status: " passed".into(),
                result_digest: "e".repeat(64),
                source: "caller_attested".into(),
                trace_sequence: Some(1),
            }],
        })
        .expect("audit");
        assert!(!audit.structurally_valid);
        for code in [
            "step_result_status_invalid",
            "step_result_error_invalid",
            "trace_identity_error",
            "delegated_check_identity_missing",
        ] {
            assert!(
                audit.findings.iter().any(|finding| finding.code == code),
                "missing {code}"
            );
        }
    }

    #[test]
    fn provenance_rejects_case_colliding_planned_steps() {
        let mut mission = report();
        let mut duplicate = mission.plan.steps[0].clone();
        duplicate.id = "ONE".into();
        mission.plan.steps.push(duplicate);
        mission.plan.step_count = 2;

        let audit = audit_execution_provenance(&ExecutionProvenanceRequest {
            mission,
            delegated_checks: vec![],
        })
        .expect("audit");
        assert!(!audit.structurally_valid);
        assert!(audit
            .findings
            .iter()
            .any(|finding| finding.code == "duplicate_planned_step"));
    }

    #[test]
    fn provenance_rejects_plan_schema_and_terminal_trace_drift() {
        let mut mission = report();
        mission.plan.execution = "planned".into();
        mission.execution_trace[1].event = "step.refused".into();
        mission.execution_trace[1].status = Some("refused".into());
        let audit = audit_execution_provenance(&ExecutionProvenanceRequest {
            mission,
            delegated_checks: vec![],
        })
        .expect("audit");
        assert!(!audit.structurally_valid);
        for code in ["plan_execution_mismatch", "trace_identity_error"] {
            assert!(
                audit.findings.iter().any(|finding| finding.code == code),
                "missing {code}"
            );
        }

        let mut invalid_schema = report();
        invalid_schema.schema_version = "legacy-mission-report/0.1".into();
        invalid_schema.execution_trace_schema_version = "legacy-trace/0.1".into();
        let audit = audit_execution_provenance(&ExecutionProvenanceRequest {
            mission: invalid_schema,
            delegated_checks: vec![],
        })
        .expect("audit");
        assert!(!audit.structurally_valid);
        assert!(audit
            .findings
            .iter()
            .any(|finding| finding.code == "mission_schema_invalid"));
        assert!(audit
            .findings
            .iter()
            .any(|finding| finding.code == "trace_schema_invalid"));
    }

    #[test]
    fn provenance_rejects_unknown_trace_event_types() {
        let mut mission = report();
        mission.execution_trace[1].event = "trace.synthetic".into();
        let audit = audit_execution_provenance(&ExecutionProvenanceRequest {
            mission,
            delegated_checks: vec![],
        })
        .expect("audit");
        assert!(!audit.structurally_valid);
        assert!(audit
            .findings
            .iter()
            .any(|finding| finding.code == "trace_identity_error"));
    }
}
