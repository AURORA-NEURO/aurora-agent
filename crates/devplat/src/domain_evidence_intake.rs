//! Bounded intake for raw tool outputs from every authoritative capability group.
//!
//! Intake is the bridge between an executed or externally supplied tool envelope and the
//! canonical domain-report/evidence layers. It records request and response JSON by exact digest,
//! retains the caller-declared outcome and claim posture, and refuses to infer meaning from a
//! response. Catalogue membership is enforced by the MCP boundary because the devplat crate does
//! not own the workspace's authoritative tool catalogue.

use crate::domain_report::{project_domain_report, validate_domain_report};
use bioprism_ids::ContentHash;
use serde_json::{json, Map, Value};
use std::collections::BTreeSet;
use thiserror::Error;

pub const DOMAIN_EVIDENCE_INTAKE_SCHEMA_VERSION: &str =
    "bioprism-devplat-domain-evidence-intake/0.1";
pub const DOMAIN_EVIDENCE_INTAKE_WORKFLOW: &str = "domain_evidence_intake";
pub const MAX_DOMAIN_EVIDENCE_INTAKE_BYTES: usize = 8 * 1024 * 1024;
pub const MAX_DOMAIN_EVIDENCE_INTAKE_DOMAINS: usize = 64;
pub const MAX_DOMAIN_EVIDENCE_INTAKE_PARENTS: usize = 128;
pub const MAX_DOMAIN_EVIDENCE_INTAKE_TEXT_BYTES: usize = 512;

const INTAKE_OUTCOMES: &[&str] = &["observed", "partial", "refused", "error", "unknown"];

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum DomainEvidenceIntakeError {
    #[error("domain evidence intake must be a JSON object")]
    NotObject,
    #[error("domain evidence intake field {0} is missing or invalid")]
    InvalidField(String),
    #[error("domain evidence intake field {field} exceeds the {maximum}-byte bound")]
    TextTooLarge { field: String, maximum: usize },
    #[error("domain evidence intake field {field} exceeds the {maximum}-item bound")]
    TooManyItems { field: String, maximum: usize },
    #[error("domain evidence intake is {actual} bytes, above the {maximum}-byte bound")]
    TooLarge { actual: usize, maximum: usize },
    #[error("domain evidence intake outcome must be one of: {0}")]
    InvalidOutcome(String),
    #[error("domain evidence intake report is invalid: {0}")]
    InvalidReport(String),
    #[error("domain evidence intake digest mismatch for {0}")]
    DigestMismatch(String),
    #[error("domain evidence intake could not be canonicalised: {0}")]
    Canonicalisation(String),
}

/// Normalize one raw request/response envelope into a canonical report-bearing intake record.
pub fn intake_domain_evidence(request: &Value) -> Result<Value, DomainEvidenceIntakeError> {
    let object = request
        .as_object()
        .ok_or(DomainEvidenceIntakeError::NotObject)?;
    ensure_size(request)?;

    let group_id = required_text(object, "group_id")?;
    let domains = text_array(object, "domains", MAX_DOMAIN_EVIDENCE_INTAKE_DOMAINS)?;
    if domains.is_empty() {
        return Err(DomainEvidenceIntakeError::InvalidField("domains".into()));
    }
    let subject_id = required_text(object, "subject_id")?;
    let source_tool = required_text(object, "source_tool")?;
    let response = object
        .get("response")
        .cloned()
        .ok_or_else(|| DomainEvidenceIntakeError::InvalidField("response".into()))?;
    let request_supplied = object.contains_key("request");
    let request_value = object.get("request").cloned().unwrap_or(Value::Null);
    let outcome = required_text(object, "outcome")?;
    if !INTAKE_OUTCOMES.contains(&outcome.as_str()) {
        return Err(DomainEvidenceIntakeError::InvalidOutcome(outcome));
    }
    let claim_posture = object
        .get("claim_posture")
        .cloned()
        .ok_or_else(|| DomainEvidenceIntakeError::InvalidField("claim_posture".into()))?;
    let parent_digests =
        digest_array(object, "parent_digests", MAX_DOMAIN_EVIDENCE_INTAKE_PARENTS)?;
    let request_digest = digest_value(&request_value)?;
    let response_digest = digest_value(&response)?;

    let observation = json!({
        "schema": DOMAIN_EVIDENCE_INTAKE_SCHEMA_VERSION,
        "workflow": DOMAIN_EVIDENCE_INTAKE_WORKFLOW,
        "group_id": group_id,
        "domains": domains,
        "subject_id": subject_id,
        "source_tool": source_tool,
        "request_supplied": request_supplied,
        "request": request_value,
        "response": response,
        "request_digest": request_digest,
        "response_digest": response_digest,
        "outcome": outcome
    });
    let intake_digest = digest_without_field(&observation, "intake_digest")?;
    let report = project_domain_report(&json!({
        "group_id": group_id,
        "domains": domains,
        "subject_id": subject_id,
        "source_tool": source_tool,
        "report": {"intake": observation},
        "claim_posture": claim_posture,
        "parent_digests": parent_digests
    }))
    .map_err(|error| DomainEvidenceIntakeError::InvalidReport(error.to_string()))?;

    let result = json!({
        "schema": DOMAIN_EVIDENCE_INTAKE_SCHEMA_VERSION,
        "workflow": DOMAIN_EVIDENCE_INTAKE_WORKFLOW,
        "group_id": group_id,
        "domains": domains,
        "subject_id": subject_id,
        "source_tool": source_tool,
        "request_supplied": request_supplied,
        "request_digest": request_digest,
        "response_digest": response_digest,
        "intake_digest": intake_digest,
        "outcome": outcome,
        "parent_digests": parent_digests,
        "report": report,
        "readiness_claimed": false,
        "execution": "not_started",
        "guarantees": [
            "request and response JSON are retained under exact canonical digests",
            "the canonical domain report preserves the caller-declared outcome and claim posture",
            "intake records a supplied envelope without executing or interpreting the source tool"
        ],
        "does_not_claim": [
            "a response or outcome label proves scientific, clinical, causal, or operational truth",
            "a source-tool membership check proves that the tool was executed or authorized",
            "intake proves provenance completeness, reproducibility, release readiness, or external effects"
        ]
    });
    ensure_size(&result)?;
    validate_domain_evidence_intake(&result)?;
    Ok(result)
}

/// Validate an intake artifact before registration, restore, or cross-process handoff.
pub fn validate_domain_evidence_intake(value: &Value) -> Result<(), DomainEvidenceIntakeError> {
    let object = value
        .as_object()
        .ok_or(DomainEvidenceIntakeError::NotObject)?;
    exact_text(object, "schema", DOMAIN_EVIDENCE_INTAKE_SCHEMA_VERSION)?;
    exact_text(object, "workflow", DOMAIN_EVIDENCE_INTAKE_WORKFLOW)?;
    let group_id = required_text(object, "group_id")?;
    let domains = text_array(object, "domains", MAX_DOMAIN_EVIDENCE_INTAKE_DOMAINS)?;
    if domains.is_empty() {
        return Err(DomainEvidenceIntakeError::InvalidField("domains".into()));
    }
    let subject_id = required_text(object, "subject_id")?;
    let source_tool = required_text(object, "source_tool")?;
    let request_supplied = object
        .get("request_supplied")
        .and_then(Value::as_bool)
        .ok_or_else(|| DomainEvidenceIntakeError::InvalidField("request_supplied".into()))?;
    let request_digest = required_digest(object, "request_digest")?;
    let response_digest = required_digest(object, "response_digest")?;
    let intake_digest = required_digest(object, "intake_digest")?;
    let outcome = required_text(object, "outcome")?;
    if !INTAKE_OUTCOMES.contains(&outcome.as_str()) {
        return Err(DomainEvidenceIntakeError::InvalidOutcome(outcome));
    }
    let parent_digests =
        digest_array(object, "parent_digests", MAX_DOMAIN_EVIDENCE_INTAKE_PARENTS)?;
    let report = object
        .get("report")
        .ok_or_else(|| DomainEvidenceIntakeError::InvalidField("report".into()))?;
    validate_domain_report(report)
        .map_err(|error| DomainEvidenceIntakeError::InvalidReport(error.to_string()))?;
    for (field, expected) in [
        ("group_id", group_id.as_str()),
        ("subject_id", subject_id.as_str()),
        ("source_tool", source_tool.as_str()),
    ] {
        if report.get(field).and_then(Value::as_str) != Some(expected) {
            return Err(DomainEvidenceIntakeError::InvalidField(format!(
                "report.{field}"
            )));
        }
    }
    if report.get("domains") != Some(&json!(domains)) {
        return Err(DomainEvidenceIntakeError::InvalidField(
            "report.domains".into(),
        ));
    }
    if report.get("parent_digests") != Some(&json!(parent_digests)) {
        return Err(DomainEvidenceIntakeError::InvalidField(
            "report.parent_digests".into(),
        ));
    }
    let observation = report
        .get("report")
        .and_then(Value::as_object)
        .and_then(|report| report.get("intake"))
        .and_then(Value::as_object)
        .ok_or_else(|| DomainEvidenceIntakeError::InvalidField("report.report.intake".into()))?;
    exact_text(observation, "schema", DOMAIN_EVIDENCE_INTAKE_SCHEMA_VERSION)?;
    exact_text(observation, "workflow", DOMAIN_EVIDENCE_INTAKE_WORKFLOW)?;
    if observation.get("group_id").and_then(Value::as_str) != Some(group_id.as_str())
        || observation.get("subject_id").and_then(Value::as_str) != Some(subject_id.as_str())
        || observation.get("source_tool").and_then(Value::as_str) != Some(source_tool.as_str())
        || observation.get("domains") != Some(&json!(domains))
    {
        return Err(DomainEvidenceIntakeError::InvalidField(
            "report.report.intake.identity".into(),
        ));
    }
    if observation.get("request_supplied") != Some(&Value::Bool(request_supplied))
        || observation.get("outcome").and_then(Value::as_str) != Some(outcome.as_str())
    {
        return Err(DomainEvidenceIntakeError::InvalidField(
            "report.report.intake.metadata".into(),
        ));
    }
    let request_value = observation.get("request").ok_or_else(|| {
        DomainEvidenceIntakeError::InvalidField("report.report.intake.request".into())
    })?;
    let response_value = observation.get("response").ok_or_else(|| {
        DomainEvidenceIntakeError::InvalidField("report.report.intake.response".into())
    })?;
    if digest_value(request_value)? != request_digest {
        return Err(DomainEvidenceIntakeError::DigestMismatch(
            "request_digest".into(),
        ));
    }
    if digest_value(response_value)? != response_digest {
        return Err(DomainEvidenceIntakeError::DigestMismatch(
            "response_digest".into(),
        ));
    }
    if observation.get("request_digest").and_then(Value::as_str) != Some(request_digest.as_str())
        || observation.get("response_digest").and_then(Value::as_str)
            != Some(response_digest.as_str())
    {
        return Err(DomainEvidenceIntakeError::DigestMismatch(
            "report.report.intake.digest".into(),
        ));
    }
    if digest_without_field(&observation.clone().into(), "intake_digest")? != intake_digest {
        return Err(DomainEvidenceIntakeError::DigestMismatch(
            "intake_digest".into(),
        ));
    }
    if object.get("readiness_claimed") != Some(&Value::Bool(false)) {
        return Err(DomainEvidenceIntakeError::InvalidField(
            "readiness_claimed".into(),
        ));
    }
    exact_text(object, "execution", "not_started")?;
    text_array(object, "guarantees", 16)?;
    text_array(object, "does_not_claim", 32)?;
    ensure_size(value)
}

fn digest_value(value: &Value) -> Result<String, DomainEvidenceIntakeError> {
    ContentHash::of_value(value)
        .map(|digest| digest.to_string())
        .map_err(|error| DomainEvidenceIntakeError::Canonicalisation(error.to_string()))
}

fn digest_without_field(value: &Value, field: &str) -> Result<String, DomainEvidenceIntakeError> {
    let mut object = value
        .as_object()
        .cloned()
        .ok_or(DomainEvidenceIntakeError::NotObject)?;
    object.remove(field);
    digest_value(&Value::Object(object))
}

fn required_digest(
    object: &Map<String, Value>,
    field: &str,
) -> Result<String, DomainEvidenceIntakeError> {
    let value = required_text(object, field)?;
    ContentHash::parse(value.clone())
        .map_err(|_| DomainEvidenceIntakeError::DigestMismatch(field.into()))?;
    Ok(value)
}

fn digest_array(
    object: &Map<String, Value>,
    field: &str,
    maximum: usize,
) -> Result<Vec<String>, DomainEvidenceIntakeError> {
    let Some(value) = object.get(field) else {
        return Ok(Vec::new());
    };
    let values = value
        .as_array()
        .ok_or_else(|| DomainEvidenceIntakeError::InvalidField(field.into()))?;
    let values = text_array_value(values, field, maximum)?;
    for value in &values {
        ContentHash::parse(value.clone())
            .map_err(|_| DomainEvidenceIntakeError::DigestMismatch(field.into()))?;
    }
    Ok(values)
}

fn text_array(
    object: &Map<String, Value>,
    field: &str,
    maximum: usize,
) -> Result<Vec<String>, DomainEvidenceIntakeError> {
    let values = object
        .get(field)
        .and_then(Value::as_array)
        .ok_or_else(|| DomainEvidenceIntakeError::InvalidField(field.into()))?;
    text_array_value(values, field, maximum)
}

fn text_array_value(
    values: &[Value],
    field: &str,
    maximum: usize,
) -> Result<Vec<String>, DomainEvidenceIntakeError> {
    if values.len() > maximum {
        return Err(DomainEvidenceIntakeError::TooManyItems {
            field: field.into(),
            maximum,
        });
    }
    let mut result = BTreeSet::new();
    for value in values {
        let text = value
            .as_str()
            .filter(|text| !text.trim().is_empty())
            .ok_or_else(|| DomainEvidenceIntakeError::InvalidField(field.into()))?;
        if text.len() > MAX_DOMAIN_EVIDENCE_INTAKE_TEXT_BYTES {
            return Err(DomainEvidenceIntakeError::TextTooLarge {
                field: field.into(),
                maximum: MAX_DOMAIN_EVIDENCE_INTAKE_TEXT_BYTES,
            });
        }
        result.insert(text.to_string());
    }
    Ok(result.into_iter().collect())
}

fn required_text(
    object: &Map<String, Value>,
    field: &str,
) -> Result<String, DomainEvidenceIntakeError> {
    let value = object
        .get(field)
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| DomainEvidenceIntakeError::InvalidField(field.into()))?;
    if value.len() > MAX_DOMAIN_EVIDENCE_INTAKE_TEXT_BYTES {
        return Err(DomainEvidenceIntakeError::TextTooLarge {
            field: field.into(),
            maximum: MAX_DOMAIN_EVIDENCE_INTAKE_TEXT_BYTES,
        });
    }
    Ok(value.to_string())
}

fn exact_text(
    object: &Map<String, Value>,
    field: &str,
    expected: &str,
) -> Result<(), DomainEvidenceIntakeError> {
    if object.get(field).and_then(Value::as_str) != Some(expected) {
        return Err(DomainEvidenceIntakeError::InvalidField(field.into()));
    }
    Ok(())
}

fn ensure_size(value: &Value) -> Result<(), DomainEvidenceIntakeError> {
    let actual = serde_json::to_vec(value)
        .map_err(|error| DomainEvidenceIntakeError::Canonicalisation(error.to_string()))?
        .len();
    if actual > MAX_DOMAIN_EVIDENCE_INTAKE_BYTES {
        return Err(DomainEvidenceIntakeError::TooLarge {
            actual,
            maximum: MAX_DOMAIN_EVIDENCE_INTAKE_BYTES,
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request() -> Value {
        json!({
            "group_id": "biological_domains",
            "domains": ["modalities", "modalities"],
            "subject_id": "subject-1",
            "source_tool": "modality_catalog",
            "request": {"modality": "single_cell"},
            "response": {"modalities": ["single_cell"], "status": "bounded"},
            "outcome": "observed",
            "claim_posture": {
                "status": "observed",
                "does_not_claim": ["clinical validity"],
                "limitations": ["caller supplied envelope"]
            }
        })
    }

    #[test]
    fn intake_is_deterministic_and_preserves_exact_digests() {
        let first = intake_domain_evidence(&request()).unwrap();
        let second = intake_domain_evidence(&request()).unwrap();
        assert_eq!(first, second);
        assert_eq!(first["readiness_claimed"], false);
        assert_eq!(first["outcome"], "observed");
        assert_eq!(
            first["report"]["report"]["intake"]["request_supplied"],
            true
        );
        assert_eq!(first["request_digest"].as_str().unwrap().len(), 64);
        assert_eq!(first["response_digest"].as_str().unwrap().len(), 64);
        validate_domain_evidence_intake(&first).unwrap();
    }

    #[test]
    fn intake_supports_missing_request_and_explicit_refusal() {
        let mut value = request();
        value.as_object_mut().unwrap().remove("request");
        value["response"] = json!({"error": "caller refused execution"});
        value["outcome"] = json!("refused");
        let result = intake_domain_evidence(&value).unwrap();
        assert_eq!(result["request_supplied"], false);
        assert_eq!(result["outcome"], "refused");
        assert_eq!(result["report"]["report"]["intake"]["request"], Value::Null);
    }

    #[test]
    fn intake_refuses_invalid_outcomes_and_tampered_nested_digests() {
        let mut invalid = request();
        invalid["outcome"] = json!("success");
        assert!(matches!(
            intake_domain_evidence(&invalid),
            Err(DomainEvidenceIntakeError::InvalidOutcome(_))
        ));

        let mut tampered = intake_domain_evidence(&request()).unwrap();
        tampered["report"]["report"]["intake"]["response"] = json!({"tampered": true});
        assert!(matches!(
            validate_domain_evidence_intake(&tampered),
            Err(DomainEvidenceIntakeError::DigestMismatch(_))
        ));
    }
}
