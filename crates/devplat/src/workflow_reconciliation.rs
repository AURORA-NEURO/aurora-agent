//! Contract-aware reconciliation for completed or retained domain workflows.
//!
//! Workflow instantiation produces a digest-bound scope and an evidence plan, while mission
//! execution produces step results, raw envelopes, trace events, and explicit omissions. This
//! module is the bounded join between those two documents. It never dispatches a tool. It checks
//! that the retained report belongs to the instantiated mission, that every result remains bound
//! to the planned step and tool, and that required evidence is distinguishable from refusal,
//! blocking, cancellation, output omission, and summary-only retention.

use bioprism_ids::ContentHash;
use serde_json::{json, Map, Value};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

use crate::evidence_bundle::verify_mission_evidence_bundle;
use crate::mission::{
    plan_mission, validate_route_review_provenance, MissionReport, MissionRequest, MissionStepPlan,
    MissionStepResult,
};

pub const DOMAIN_WORKFLOW_RECONCILE_SCHEMA_VERSION: &str =
    "bioprism-devplat-domain-workflow-reconcile/0.1";
pub const MAX_DOMAIN_WORKFLOW_RECONCILE_BYTES: usize = 20_000_000;
pub const MAX_DOMAIN_WORKFLOW_RECONCILE_STEPS: usize = 128;
pub const MAX_DOMAIN_WORKFLOW_RECONCILE_FINDINGS: usize = 512;

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum DomainWorkflowReconcileError {
    #[error("workflow reconciliation request must be an object")]
    RequestNotObject,
    #[error("workflow reconciliation request is invalid: {0}")]
    InvalidRequest(String),
    #[error("workflow reconciliation input is {actual} bytes; maximum is {maximum}")]
    TooLarge { actual: usize, maximum: usize },
    #[error("workflow reconciliation document could not be canonicalised: {0}")]
    Canonicalisation(String),
    #[error("workflow evidence source is invalid: {0}")]
    InvalidEvidenceSource(String),
}

fn checked_bytes(value: &Value) -> Result<(), DomainWorkflowReconcileError> {
    let encoded = serde_json::to_vec(value)
        .map_err(|error| DomainWorkflowReconcileError::Canonicalisation(error.to_string()))?;
    if encoded.len() > MAX_DOMAIN_WORKFLOW_RECONCILE_BYTES {
        return Err(DomainWorkflowReconcileError::TooLarge {
            actual: encoded.len(),
            maximum: MAX_DOMAIN_WORKFLOW_RECONCILE_BYTES,
        });
    }
    Ok(())
}

fn digest(value: &Value) -> Result<String, DomainWorkflowReconcileError> {
    ContentHash::of_value(value)
        .map(|hash| hash.to_string())
        .map_err(|error| DomainWorkflowReconcileError::Canonicalisation(error.to_string()))
}

fn text_field<'a>(
    object: &'a Map<String, Value>,
    field: &str,
) -> Result<&'a str, DomainWorkflowReconcileError> {
    object
        .get(field)
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| {
            DomainWorkflowReconcileError::InvalidRequest(format!(
                "{field} must be a non-empty string"
            ))
        })
}

fn require_digest(
    object: &Map<String, Value>,
    field: &str,
) -> Result<String, DomainWorkflowReconcileError> {
    let value = text_field(object, field)?;
    ContentHash::parse(value.to_string()).map_err(|_| {
        DomainWorkflowReconcileError::InvalidRequest(format!(
            "{field} must be a lowercase 64-character SHA-256 digest"
        ))
    })?;
    Ok(value.to_string())
}

fn finding(code: &str, severity: &str, message: impl Into<String>, step_id: Option<&str>) -> Value {
    json!({
        "code": code,
        "severity": severity,
        "message": message.into(),
        "step_id": step_id,
    })
}

fn append_finding(findings: &mut Vec<Value>, value: Value) {
    if findings.len() < MAX_DOMAIN_WORKFLOW_RECONCILE_FINDINGS {
        findings.push(value);
    }
}

fn step_contract_ids(
    instantiation: &Value,
) -> Result<BTreeSet<String>, DomainWorkflowReconcileError> {
    let steps = instantiation
        .get("evidence_plan")
        .and_then(|plan| plan.get("steps"))
        .and_then(Value::as_array)
        .ok_or_else(|| {
            DomainWorkflowReconcileError::InvalidRequest(
                "instantiation.evidence_plan.steps must be an array".into(),
            )
        })?;
    if steps.len() > MAX_DOMAIN_WORKFLOW_RECONCILE_STEPS {
        return Err(DomainWorkflowReconcileError::InvalidRequest(format!(
            "evidence plan contains more than {MAX_DOMAIN_WORKFLOW_RECONCILE_STEPS} steps"
        )));
    }
    let mut ids = BTreeSet::new();
    for (index, step) in steps.iter().enumerate() {
        let id = step
            .get("step_id")
            .and_then(Value::as_str)
            .filter(|value| !value.trim().is_empty())
            .ok_or_else(|| {
                DomainWorkflowReconcileError::InvalidRequest(format!(
                    "instantiation.evidence_plan.steps[{index}].step_id must be a non-empty string"
                ))
            })?;
        if !ids.insert(id.to_string()) {
            return Err(DomainWorkflowReconcileError::InvalidRequest(format!(
                "evidence plan contains duplicate step id {id:?}"
            )));
        }
    }
    Ok(ids)
}

fn trace_audit(report: &MissionReport, findings: &mut Vec<Value>) -> Value {
    let contiguous = report
        .execution_trace
        .iter()
        .enumerate()
        .all(|(expected, event)| event.sequence == expected);
    if !contiguous {
        append_finding(
            findings,
            finding(
                "trace_sequence_not_contiguous",
                "error",
                "execution trace sequence numbers are not contiguous from zero",
                None,
            ),
        );
    }
    let starts = report
        .execution_trace
        .first()
        .map(|event| event.event == "mission.started")
        .unwrap_or(false);
    let ends = report
        .execution_trace
        .last()
        .map(|event| event.event == "mission.completed")
        .unwrap_or(false);
    if !starts || !ends {
        append_finding(
            findings,
            finding(
                "trace_lifecycle_incomplete",
                "error",
                "execution trace must begin with mission.started and end with mission.completed",
                None,
            ),
        );
    }
    json!({
        "present": !report.execution_trace.is_empty(),
        "event_count": report.execution_trace.len(),
        "contiguous": contiguous,
        "starts_with_mission_started": starts,
        "ends_with_mission_completed": ends,
    })
}

fn result_digest(wire: Option<&Value>) -> Result<Option<String>, DomainWorkflowReconcileError> {
    wire.map(digest).transpose()
}

fn evidence_state(status: &str, retained: bool) -> &'static str {
    match status {
        "succeeded" if retained => "completed_output_retained",
        "succeeded" => "completed_output_omitted",
        "refused" => "explicit_refusal",
        "blocked" => "explicit_block",
        "cancelled" => "explicit_cancellation",
        "planned" => "not_executed",
        _ => "unknown_status",
    }
}

fn result_row(
    result: &MissionStepResult,
    expected: &MissionStepPlan,
    findings: &mut Vec<Value>,
) -> Result<Value, DomainWorkflowReconcileError> {
    if result.tool != expected.tool {
        append_finding(
            findings,
            finding(
                "result_tool_mismatch",
                "error",
                format!(
                    "result tool `{}` does not match planned tool `{}`",
                    result.tool, expected.tool
                ),
                Some(&result.id),
            ),
        );
    }
    if result.required != expected.required {
        append_finding(
            findings,
            finding(
                "result_requiredness_mismatch",
                "error",
                "result requiredness does not match the planned step",
                Some(&result.id),
            ),
        );
    }
    let valid_status = matches!(
        result.status.as_str(),
        "succeeded" | "refused" | "blocked" | "cancelled" | "planned"
    );
    if !valid_status {
        append_finding(
            findings,
            finding(
                "unknown_result_status",
                "error",
                format!(
                    "result status `{}` is not part of the mission contract",
                    result.status
                ),
                Some(&result.id),
            ),
        );
    }
    if let Some(arguments_digest) = result.arguments_digest.as_deref() {
        if ContentHash::parse(arguments_digest.to_string()).is_err() {
            append_finding(
                findings,
                finding(
                    "arguments_digest_invalid",
                    "error",
                    "result arguments_digest is not a lowercase SHA-256 digest",
                    Some(&result.id),
                ),
            );
        }
    }
    let retained = result.wire.is_some();
    let computed_result_digest = result_digest(result.wire.as_ref())?;
    Ok(json!({
        "step_id": result.id,
        "tool": result.tool,
        "required": result.required,
        "status": result.status,
        "arguments_digest": result.arguments_digest,
        "bytes": result.bytes,
        "result_retained": retained,
        "result_digest": computed_result_digest,
        "error": result.error,
        "evidence_state": evidence_state(&result.status, retained),
        "planned": {
            "tool": expected.tool,
            "required": expected.required,
            "wave": expected.wave,
        },
    }))
}

fn route_review_audit(
    expected: Option<&Value>,
    observed: Option<&Value>,
    findings: &mut Vec<Value>,
) -> Value {
    match (expected, observed) {
        (None, None) => json!({
            "present": false,
            "status": "absent",
            "matched": true,
            "expected": Value::Null,
            "observed": Value::Null,
        }),
        (Some(expected), None) => {
            append_finding(
                findings,
                finding(
                    "route_review_provenance_missing",
                    "error",
                    "the retained mission plan dropped route-review provenance carried by the instantiated workflow",
                    None,
                ),
            );
            json!({
                "present": true,
                "status": "missing",
                "matched": false,
                "expected": expected,
                "observed": Value::Null,
            })
        }
        (None, Some(observed)) => {
            append_finding(
                findings,
                finding(
                    "route_review_provenance_unexpected",
                    "error",
                    "the retained mission plan introduced route-review provenance that was not present at workflow instantiation",
                    None,
                ),
            );
            json!({
                "present": true,
                "status": "unexpected",
                "matched": false,
                "expected": Value::Null,
                "observed": observed,
            })
        }
        (Some(expected), Some(observed)) => {
            let expected_error = validate_route_review_provenance(expected).err();
            let observed_error = validate_route_review_provenance(observed).err();
            if let Some(reason) = expected_error.as_deref() {
                append_finding(
                    findings,
                    finding(
                        "route_review_provenance_expected_invalid",
                        "error",
                        format!("instantiated route-review provenance is invalid: {reason}"),
                        None,
                    ),
                );
            }
            if let Some(reason) = observed_error.as_deref() {
                append_finding(
                    findings,
                    finding(
                        "route_review_provenance_invalid",
                        "error",
                        format!("retained route-review provenance is invalid: {reason}"),
                        None,
                    ),
                );
            }
            let matched =
                expected_error.is_none() && observed_error.is_none() && expected == observed;
            if !matched && expected_error.is_none() && observed_error.is_none() {
                append_finding(
                    findings,
                    finding(
                        "route_review_provenance_mismatch",
                        "error",
                        "retained route-review identity or evidence posture does not match the instantiated workflow",
                        None,
                    ),
                );
            }
            json!({
                "present": true,
                "status": if matched { "matched" } else { "mismatch" },
                "matched": matched,
                "expected": expected,
                "observed": observed,
            })
        }
    }
}

fn reconcile_report(
    report_value: &Value,
    expected_request: &MissionRequest,
    expected_plan: &crate::mission::MissionPlan,
    findings: &mut Vec<Value>,
) -> Result<(MissionReport, Vec<Value>, Value), DomainWorkflowReconcileError> {
    let report_object = report_value.as_object().ok_or_else(|| {
        DomainWorkflowReconcileError::InvalidEvidenceSource(
            "mission_report must be an object".into(),
        )
    })?;
    if report_object.get("workflow").and_then(Value::as_str) != Some("agent_mission") {
        append_finding(
            findings,
            finding(
                "mission_workflow_mismatch",
                "error",
                "mission report workflow must be agent_mission",
                None,
            ),
        );
    }
    let report: MissionReport = serde_json::from_value(report_value.clone()).map_err(|error| {
        DomainWorkflowReconcileError::InvalidEvidenceSource(format!(
            "mission_report does not match MissionReport: {error}"
        ))
    })?;
    if report.plan.mission_id != expected_request.mission_id {
        append_finding(
            findings,
            finding(
                "mission_id_mismatch",
                "error",
                format!(
                    "mission report belongs to `{}`, expected `{}`",
                    report.plan.mission_id, expected_request.mission_id
                ),
                None,
            ),
        );
    }
    if report.plan.digest != expected_plan.digest {
        append_finding(
            findings,
            finding(
                "mission_plan_digest_mismatch",
                "error",
                format!(
                    "mission plan digest `{}` does not match instantiated digest `{}`",
                    report.plan.digest, expected_plan.digest
                ),
                None,
            ),
        );
    }
    if !expected_request.policy.execute && report.execution == "executed" {
        append_finding(
            findings,
            finding(
                "execution_posture_mismatch",
                "error",
                "a plan-only instantiation cannot reconcile an executed mission report",
                None,
            ),
        );
    }
    let route_review = route_review_audit(
        expected_plan.route_review_provenance.as_ref(),
        report.plan.route_review_provenance.as_ref(),
        findings,
    );

    let expected_by_id = expected_plan
        .steps
        .iter()
        .map(|step| (step.id.as_str(), step))
        .collect::<BTreeMap<_, _>>();
    let mut seen = BTreeSet::new();
    let mut rows_by_id = BTreeMap::new();
    for result in &report.results {
        if !seen.insert(result.id.clone()) {
            append_finding(
                findings,
                finding(
                    "duplicate_result",
                    "error",
                    "mission report contains duplicate result rows",
                    Some(&result.id),
                ),
            );
            continue;
        }
        let Some(expected) = expected_by_id.get(result.id.as_str()) else {
            append_finding(
                findings,
                finding(
                    "unknown_result_step",
                    "error",
                    "mission report contains a result for an unplanned step",
                    Some(&result.id),
                ),
            );
            continue;
        };
        rows_by_id.insert(result.id.clone(), result_row(result, expected, findings)?);
    }

    let mut rows = Vec::with_capacity(expected_plan.steps.len());
    let mut required_success = true;
    let mut required_retained = true;
    let mut optional_unresolved = false;
    for expected in &expected_plan.steps {
        if let Some(row) = rows_by_id.get(&expected.id) {
            let status = row["status"].as_str().unwrap_or("unknown");
            let retained = row["result_retained"].as_bool().unwrap_or(false);
            if expected.required && status != "succeeded" {
                required_success = false;
                append_finding(
                    findings,
                    finding(
                        "required_step_not_successful",
                        "error",
                        format!("required step has status `{status}`"),
                        Some(&expected.id),
                    ),
                );
            }
            if expected.required && !retained {
                required_retained = false;
                if status == "succeeded" {
                    append_finding(
                        findings,
                        finding(
                            "required_output_omitted",
                            "warning",
                            "required step succeeded but its raw output was not retained",
                            Some(&expected.id),
                        ),
                    );
                }
            }
            if !expected.required && status != "succeeded" {
                optional_unresolved = true;
            }
            rows.push(row.clone());
        } else {
            required_success = required_success && !expected.required;
            if expected.required {
                required_retained = false;
            } else {
                optional_unresolved = true;
            }
            append_finding(
                findings,
                finding(
                    if expected.required {
                        "required_step_result_missing"
                    } else {
                        "optional_step_result_missing"
                    },
                    if expected.required {
                        "error"
                    } else {
                        "warning"
                    },
                    "mission report does not contain a result row for the planned step",
                    Some(&expected.id),
                ),
            );
            rows.push(json!({
                "step_id": expected.id,
                "tool": expected.tool,
                "required": expected.required,
                "status": "missing",
                "arguments_digest": Value::Null,
                "bytes": 0,
                "result_retained": false,
                "result_digest": Value::Null,
                "error": Value::Null,
                "evidence_state": "missing_step_result",
                "planned": {"tool": expected.tool, "required": expected.required, "wave": expected.wave},
            }));
        }
    }

    let counts = [
        ("succeeded", report.succeeded),
        ("refused", report.refused),
        ("blocked", report.blocked),
        ("cancelled", report.cancelled),
    ];
    for (status, reported) in counts {
        let observed = report
            .results
            .iter()
            .filter(|result| result.status == status)
            .count();
        if observed != reported {
            append_finding(
                findings,
                finding(
                    "mission_counter_mismatch",
                    "error",
                    format!(
                        "mission reports {reported} `{status}` results but contains {observed}"
                    ),
                    None,
                ),
            );
        }
    }
    let trace = trace_audit(&report, findings);
    Ok((
        report,
        rows,
        json!({
            "required_success": required_success,
            "required_outputs_retained": required_retained,
            "optional_unresolved": optional_unresolved,
            "trace": trace,
            "route_review": route_review,
        }),
    ))
}

/// Reconcile an instantiated workflow against a retained `agent_mission` report or evidence
/// bundle. This is an audit projection only: it never dispatches, retries, or upgrades evidence
/// into a scientific, clinical, operational, regulatory, or release claim.
pub fn reconcile_domain_workflow(request: &Value) -> Result<Value, DomainWorkflowReconcileError> {
    checked_bytes(request)?;
    let object = request
        .as_object()
        .ok_or(DomainWorkflowReconcileError::RequestNotObject)?;
    let instantiation = object.get("instantiation").ok_or_else(|| {
        DomainWorkflowReconcileError::InvalidRequest("instantiation is required".into())
    })?;
    let instantiation_object = instantiation.as_object().ok_or_else(|| {
        DomainWorkflowReconcileError::InvalidRequest("instantiation must be an object".into())
    })?;
    if instantiation_object.get("workflow").and_then(Value::as_str)
        != Some("domain_workflow_instantiate")
    {
        return Err(DomainWorkflowReconcileError::InvalidRequest(
            "instantiation.workflow must be domain_workflow_instantiate".into(),
        ));
    }
    if instantiation_object.get("ok").and_then(Value::as_bool) != Some(true) {
        return Err(DomainWorkflowReconcileError::InvalidRequest(
            "instantiation must be an accepted workflow report".into(),
        ));
    }
    let workflow_id = text_field(instantiation_object, "workflow_id")?.to_string();
    let workflow_digest = require_digest(instantiation_object, "workflow_digest")?;
    let catalog_digest = require_digest(instantiation_object, "catalog_digest")?;
    let domain_contract_digest = require_digest(instantiation_object, "domain_contract_digest")?;
    let mission_value = instantiation_object.get("mission").ok_or_else(|| {
        DomainWorkflowReconcileError::InvalidRequest("instantiation.mission is required".into())
    })?;
    let expected_request: MissionRequest =
        serde_json::from_value(mission_value.clone()).map_err(|error| {
            DomainWorkflowReconcileError::InvalidRequest(format!(
                "invalid instantiated mission: {error}"
            ))
        })?;
    let expected_plan = plan_mission(&expected_request).map_err(|error| {
        DomainWorkflowReconcileError::InvalidRequest(format!(
            "instantiated mission cannot be planned: {error}"
        ))
    })?;
    let contract_step_ids = step_contract_ids(instantiation)?;
    let expected_step_ids = expected_plan
        .steps
        .iter()
        .map(|step| step.id.clone())
        .collect::<BTreeSet<_>>();
    let mut findings = Vec::new();
    if contract_step_ids != expected_step_ids {
        append_finding(
            &mut findings,
            finding(
                "evidence_plan_scope_mismatch",
                "error",
                "instantiation evidence plan does not cover exactly the planned step ids",
                None,
            ),
        );
    }

    let report_input = object.get("mission_report");
    let bundle_input = object.get("evidence_bundle");
    if report_input.is_none() && bundle_input.is_none() {
        return Err(DomainWorkflowReconcileError::InvalidRequest(
            "one of mission_report or evidence_bundle is required".into(),
        ));
    }
    let mut source = "mission_report";
    let mut bundle_verification = Value::Null;
    let mut bundle_retention = Value::Null;
    let mut report_value = report_input.cloned();
    if let Some(bundle) = bundle_input {
        let verification = verify_mission_evidence_bundle(bundle).map_err(|error| {
            DomainWorkflowReconcileError::InvalidEvidenceSource(error.to_string())
        })?;
        let bundle_object = bundle.as_object().ok_or_else(|| {
            DomainWorkflowReconcileError::InvalidEvidenceSource(
                "evidence_bundle must be an object".into(),
            )
        })?;
        let bundle_mission_id = text_field(bundle_object, "mission_id")?;
        if bundle_mission_id != expected_request.mission_id {
            append_finding(
                &mut findings,
                finding(
                    "bundle_mission_id_mismatch",
                    "error",
                    format!(
                        "evidence bundle belongs to `{bundle_mission_id}`, expected `{}`",
                        expected_request.mission_id
                    ),
                    None,
                ),
            );
        }
        bundle_retention = bundle_object
            .get("retention")
            .cloned()
            .unwrap_or(Value::Null);
        bundle_verification = verification;
        if report_value.is_none() {
            if bundle_object
                .get("result")
                .is_some_and(|result| result.is_object())
            {
                report_value = bundle_object.get("result").cloned();
                source = "evidence_bundle_full";
            } else {
                source = "evidence_bundle_summary_only";
            }
        } else {
            source = "mission_report_and_evidence_bundle";
        }
    }

    let mut evidence_rows = Vec::new();
    let mut report_summary = json!({
        "present": false,
        "mission_status": Value::Null,
        "execution": Value::Null,
        "result_count": 0,
            "returned_bytes": 0,
        "route_review_provenance": expected_plan.route_review_provenance.clone().unwrap_or(Value::Null),
    });
    let mut evidence_summary = json!({
        "required_success": false,
        "required_outputs_retained": false,
        "optional_unresolved": false,
        "trace": Value::Null,
        "route_review": {
            "present": expected_plan.route_review_provenance.is_some(),
            "status": if expected_plan.route_review_provenance.is_some() { "not_observable" } else { "absent" },
            "matched": Value::Null,
            "expected": expected_plan.route_review_provenance.clone().unwrap_or(Value::Null),
            "observed": Value::Null,
        },
    });
    if let Some(report_value) = report_value {
        let (report, rows, summary) = reconcile_report(
            &report_value,
            &expected_request,
            &expected_plan,
            &mut findings,
        )?;
        evidence_rows = rows;
        evidence_summary = summary;
        report_summary = json!({
            "present": true,
            "mission_status": report.mission_status,
            "execution": report.execution,
            "result_count": report.results.len(),
            "succeeded": report.succeeded,
            "refused": report.refused,
            "blocked": report.blocked,
            "cancelled": report.cancelled,
            "required_failures": report.required_failures,
            "returned_bytes": report.returned_bytes,
            "route_review_provenance": report.plan.route_review_provenance,
        });
    } else {
        for expected in &expected_plan.steps {
            evidence_rows.push(json!({
                "step_id": expected.id,
                "tool": expected.tool,
                "required": expected.required,
                "status": "not_retained",
                "arguments_digest": Value::Null,
                "bytes": 0,
                "result_retained": false,
                "result_digest": Value::Null,
                "error": Value::Null,
                "evidence_state": "summary_only_result_omitted",
                "planned": {"tool": expected.tool, "required": expected.required, "wave": expected.wave},
            }));
        }
        append_finding(
            &mut findings,
            finding(
                "mission_result_not_retained",
                "warning",
                "summary-only evidence cannot reconcile per-step completion or output evidence",
                None,
            ),
        );
    }

    let integrity_valid = findings
        .iter()
        .all(|finding| finding["severity"] != "error")
        && bundle_verification
            .get("valid")
            .and_then(Value::as_bool)
            .unwrap_or(true);
    let required_success = evidence_summary["required_success"]
        .as_bool()
        .unwrap_or(false);
    let required_outputs_retained = evidence_summary["required_outputs_retained"]
        .as_bool()
        .unwrap_or(false);
    let optional_unresolved = evidence_summary["optional_unresolved"]
        .as_bool()
        .unwrap_or(false);
    let evidence_valid = report_summary["present"].as_bool().unwrap_or(false)
        && required_success
        && required_outputs_retained
        && integrity_valid;
    let completion_status = if !report_summary["present"].as_bool().unwrap_or(false) {
        "unverified"
    } else if !required_success {
        "failed"
    } else if !required_outputs_retained {
        "complete_with_output_omissions"
    } else if optional_unresolved {
        "partial"
    } else {
        "complete"
    };

    let mut output = json!({
        "ok": true,
        "schema": DOMAIN_WORKFLOW_RECONCILE_SCHEMA_VERSION,
        "workflow": "domain_workflow_reconcile",
        "workflow_id": workflow_id,
        "workflow_digest": workflow_digest,
        "catalog_digest": catalog_digest,
        "domain_contract_digest": domain_contract_digest,
        "mission_id": expected_request.mission_id,
        "mission_plan_digest": expected_plan.digest,
        "route_review_provenance": expected_plan.route_review_provenance.clone().unwrap_or(Value::Null),
        "route_review_integrity": evidence_summary["route_review"].clone(),
        "source": source,
        "retention": bundle_retention,
        "bundle_verification": bundle_verification,
        "report": report_summary,
        "evidence": {
            "rows": evidence_rows,
            "required_success": required_success,
            "required_outputs_retained": required_outputs_retained,
            "optional_unresolved": optional_unresolved,
            "evidence_valid": evidence_valid,
        },
        "completion": {
            "status": completion_status,
            "ready": evidence_valid,
            "review_required": true,
            "claims_posture": if evidence_valid { "review_required_before_claims" } else { "claims_not_supported" },
        },
        "integrity": {
            "valid": integrity_valid,
            "finding_count": findings.len(),
            "findings": findings,
        },
        "execution": "not_started",
        "guarantees": [
            "workflow and mission plan digests are correlated before evidence is assessed",
            "every planned step remains visible as retained, omitted, refused, blocked, cancelled, or missing",
            "raw result retention is distinguished from successful status",
            "summary-only bundles never become completion evidence",
            "reconciliation never dispatches, retries, or mutates mission state",
        ],
        "limitations": [
            "completion is a structural evidence posture, not scientific, clinical, causal, operational, regulatory, or release truth",
            "a retained raw tool envelope does not prove the domain semantics of its payload",
            "external signatures, provider identity, and durable storage authority remain separate obligations",
        ],
        "links": {
            "workflow_catalogue": "/v1/domain-workflows",
            "workflow_instantiate": "/v1/domain-workflows/instantiate",
            "mission": format!("/v1/missions/{}", expected_request.mission_id),
            "evidence_bundle": format!("/v1/missions/{}/evidence-bundle", expected_request.mission_id),
        },
    });
    let reconciliation_digest = digest(&output)?;
    output["reconciliation_digest"] = Value::String(reconciliation_digest);
    checked_bytes(&output)?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workflow::instantiate_domain_workflow;

    fn inputs() -> (Value, Value) {
        (
            json!([{
                "id":"oncology_workflows",
                "domains":["oncology"],
                "crates":["bioprism-onco"],
                "mcp_tools":["onco_boundary_check"],
                "cli_entrypoints":[],
                "status":"available"
            }]),
            json!([{"name":"onco_boundary_check","inputSchema":{"type":"object"}}]),
        )
    }

    fn instantiation() -> Value {
        let (catalogue, tools) = inputs();
        instantiate_domain_workflow(
            &catalogue,
            &tools,
            &json!({
                "workflow_id":"oncology_workflows",
                "mission_id":"reconcile-1",
                "goal":"review the oncology boundary",
                "steps":[{"id":"boundary","tool":"onco_boundary_check","arguments":{}}],
                "policy":{"execute":true}
            }),
        )
        .unwrap()
    }

    fn reviewed_instantiation() -> Value {
        let (catalogue, tools) = inputs();
        let base = instantiation();
        let steps = base["mission"]["steps"].clone();
        instantiate_domain_workflow(
            &catalogue,
            &tools,
            &json!({
                "workflow_id":"oncology_workflows",
                "mission_id":"reconcile-reviewed-1",
                "goal":"review the oncology boundary",
                "steps":steps.clone(),
                "policy":{"execute":true},
                "route_review": {
                    "ok": true,
                    "workflow": "capability_route_review",
                    "review_id": "a".repeat(64),
                    "route_id": "b".repeat(64),
                    "catalog_digest": "c".repeat(64),
                    "goal": "review the oncology boundary",
                    "findings": [],
                    "review_status": "ready",
                    "handoff_status": "mission_preflight_required",
                    "mission_draft": {
                        "goal":"review the oncology boundary",
                        "steps":steps,
                        "dependency_waves":[["boundary"]]
                    },
                    "execution":"not_started"
                }
            }),
        )
        .unwrap()
    }

    fn report(instantiation: &Value, status: &str, wire: Option<Value>) -> Value {
        let request: MissionRequest =
            serde_json::from_value(instantiation["mission"].clone()).unwrap();
        let plan = plan_mission(&request).unwrap();
        json!({
            "ok": true,
            "workflow": "agent_mission",
            "schema_version": "bioprism-devplat-mission/0.1",
            "plan": serde_json::to_value(plan).unwrap(),
            "execution": "executed",
            "mission_status": if status == "succeeded" { "succeeded" } else { "failed" },
            "succeeded": usize::from(status == "succeeded"),
            "refused": usize::from(status == "refused"),
            "blocked": usize::from(status == "blocked"),
            "cancelled": usize::from(status == "cancelled"),
            "required_failures": usize::from(status != "succeeded"),
            "returned_bytes": 12,
            "results": [{
                "id":"boundary",
                "tool":"onco_boundary_check",
                "status":status,
                "required":true,
                "arguments_digest":"a".repeat(64),
                "bytes":12,
                "wire":wire,
                "error": if status == "succeeded" { Value::Null } else { json!("refused") }
            }],
            "execution_trace_schema_version":"bioprism-devplat-mission-trace/0.1",
            "execution_trace":[
                {"sequence":0,"event":"mission.started","wave":null,"step_id":null,"tool":null,"status":"running","arguments_digest":null,"bytes":0,"detail":null},
                {"sequence":1,"event":"mission.completed","wave":null,"step_id":null,"tool":null,"status":if status == "succeeded" { "succeeded" } else { "failed" },"arguments_digest":null,"bytes":12,"detail":null}
            ],
            "claim_requests":[],
            "claim_lineage":{},
            "guarantees":[],
            "limitations":[]
        })
    }

    #[test]
    fn reconciles_retained_success_into_complete_evidence() {
        let instantiation = instantiation();
        let output = reconcile_domain_workflow(&json!({
            "instantiation": instantiation,
            "mission_report": report(&instantiation, "succeeded", Some(json!({"result": {"ok": true}})))
        })).unwrap();
        assert_eq!(output["completion"]["status"], "complete");
        assert_eq!(output["completion"]["ready"], true);
        assert_eq!(
            output["evidence"]["rows"][0]["evidence_state"],
            "completed_output_retained"
        );
        assert!(output["reconciliation_digest"].is_string());
    }

    #[test]
    fn keeps_refusal_and_output_omission_explicit() {
        let instantiation = instantiation();
        let refused = reconcile_domain_workflow(&json!({
            "instantiation": instantiation,
            "mission_report": report(&instantiation, "refused", None)
        }))
        .unwrap();
        assert_eq!(refused["completion"]["status"], "failed");
        assert_eq!(
            refused["evidence"]["rows"][0]["evidence_state"],
            "explicit_refusal"
        );

        let omitted = reconcile_domain_workflow(&json!({
            "instantiation": instantiation,
            "mission_report": report(&instantiation, "succeeded", None)
        }))
        .unwrap();
        assert_eq!(
            omitted["completion"]["status"],
            "complete_with_output_omissions"
        );
        assert_eq!(omitted["completion"]["ready"], false);
    }

    #[test]
    fn refuses_tampered_plan_and_summary_only_bundle_cannot_complete() {
        let instantiation = instantiation();
        let mut tampered = report(&instantiation, "succeeded", Some(json!({"ok": true})));
        tampered["plan"]["digest"] = json!("b".repeat(64));
        let reconciled = reconcile_domain_workflow(&json!({
            "instantiation": instantiation,
            "mission_report": tampered
        }))
        .unwrap();
        assert_eq!(reconciled["integrity"]["valid"], false);
        assert!(reconciled["integrity"]["findings"]
            .as_array()
            .unwrap()
            .iter()
            .any(|row| row["code"] == "mission_plan_digest_mismatch"));
    }

    #[test]
    fn route_review_provenance_must_match_the_instantiated_workflow() {
        let instantiation = reviewed_instantiation();
        let mut tampered = report(&instantiation, "succeeded", Some(json!({"ok": true})));
        tampered["plan"]["route_review_provenance"]["route_id"] = json!("d".repeat(64));
        let reconciled = reconcile_domain_workflow(&json!({
            "instantiation": instantiation,
            "mission_report": tampered
        }))
        .unwrap();
        assert_eq!(reconciled["route_review_integrity"]["matched"], false);
        assert_eq!(reconciled["integrity"]["valid"], false);
        assert!(reconciled["integrity"]["findings"]
            .as_array()
            .unwrap()
            .iter()
            .any(|row| row["code"] == "route_review_provenance_mismatch"));
    }

    #[test]
    fn summary_only_bundle_is_verified_but_cannot_prove_step_completion() {
        let instantiation = instantiation();
        let mut bundle = json!({
            "schema": "bioprism-api/mission-evidence-bundle/0.1",
            "workflow": "mission_evidence_bundle_export",
            "mission_id": "reconcile-1",
            "retention": {
                "mode": "summary_only",
                "result_retained": false,
                "result_included": false,
                "summary_retained": true,
                "result_omitted": {"bytes": 12, "sha256": "a".repeat(64)}
            },
            "result": Value::Null,
            "result_digest": "b".repeat(64),
            "trace": [],
            "export": {
                "format": "json",
                "include_result": false,
                "include_trace": false,
                "trace_included": false,
                "include_fixtures": false,
                "max_items": 128,
                "execution": "not_started",
                "digest_algorithm": "sha256"
            }
        });
        let bundle_digest = ContentHash::of_value(&bundle).unwrap().to_string();
        bundle["bundle_digest"] = json!(bundle_digest);
        let output = reconcile_domain_workflow(&json!({
            "instantiation": instantiation,
            "evidence_bundle": bundle
        }))
        .unwrap();
        assert_eq!(output["bundle_verification"]["valid"], true);
        assert_eq!(output["integrity"]["valid"], true);
        assert_eq!(output["source"], "evidence_bundle_summary_only");
        assert_eq!(output["completion"]["status"], "unverified");
        assert_eq!(output["completion"]["ready"], false);
        assert_eq!(
            output["evidence"]["rows"][0]["evidence_state"],
            "summary_only_result_omitted"
        );
    }
}
