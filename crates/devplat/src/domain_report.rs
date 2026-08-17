//! Explicit, bounded projections for reports emitted by any capability group.
//!
//! A domain report is deliberately a projection boundary, not an execution engine.  It gives
//! callers one stable envelope for retaining a report from an ingestion, modelling, evaluation,
//! safety, release, or developer workflow while keeping the source tool, capability group,
//! limitations, and non-claims adjacent to the payload.  The envelope is useful for indexing and
//! later routing; it does not turn a caller supplied JSON object into scientific or operational
//! truth.

use bioprism_ids::ContentHash;
use serde_json::{json, Map, Value};
use std::collections::BTreeSet;
use thiserror::Error;

pub const DOMAIN_REPORT_SCHEMA_VERSION: &str = "bioprism-devplat-domain-report/0.1";
pub const DOMAIN_REPORT_PROJECT_SCHEMA_VERSION: &str = "bioprism-devplat-domain-report-project/0.1";
pub const DOMAIN_REPORT_COVERAGE_SCHEMA_VERSION: &str =
    "bioprism-devplat-domain-report-coverage/0.1";
pub const DOMAIN_REPORT_PROJECT_WORKFLOW: &str = "domain_report_project";
pub const DOMAIN_REPORT_COVERAGE_WORKFLOW: &str = "domain_report_coverage";
pub const ADAPTER_DOMAIN_REPORT_SCHEMA_VERSION: &str = "bioprism-devplat-adapter-domain-report/0.1";
pub const ADAPTER_DOMAIN_REPORT_WORKFLOW: &str = "adapter_domain_report";
pub const PROVIDER_DOMAIN_REPORT_SCHEMA_VERSION: &str =
    "bioprism-devplat-provider-domain-report/0.1";
pub const PROVIDER_DOMAIN_REPORT_WORKFLOW: &str = "provider_domain_report";
pub const MAX_DOMAIN_REPORT_BYTES: usize = 8 * 1024 * 1024;
pub const MAX_DOMAIN_REPORT_PARENTS: usize = 128;
pub const MAX_DOMAIN_REPORT_DOMAINS: usize = 64;
pub const MAX_DOMAIN_REPORT_NON_CLAIMS: usize = 64;
pub const MAX_DOMAIN_REPORT_LIMITATIONS: usize = 64;
pub const MAX_DOMAIN_REPORT_TEXT_BYTES: usize = 512;

const CLAIM_STATUSES: &[&str] = &[
    "observed",
    "derived",
    "review_required",
    "refused",
    "not_applicable",
];

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum DomainReportError {
    #[error("domain report must be a JSON object")]
    NotObject,
    #[error("domain report field {0} is missing or invalid")]
    InvalidField(String),
    #[error("domain report field {field} exceeds the {maximum}-byte bound")]
    TextTooLarge { field: String, maximum: usize },
    #[error("domain report field {field} exceeds the {maximum}-item bound")]
    TooManyItems { field: String, maximum: usize },
    #[error("domain report is {actual} bytes, above the {maximum}-byte bound")]
    TooLarge { actual: usize, maximum: usize },
    #[error("domain report digest {field} is not a lowercase 64-character SHA-256 digest")]
    InvalidDigest { field: String },
    #[error("domain report claim status must be one of: {0}")]
    InvalidClaimStatus(String),
    #[error("domain report could not be canonicalised: {0}")]
    Canonicalisation(String),
}

/// Build the canonical report envelope from a caller-owned projection request.
pub fn project_domain_report(request: &Value) -> Result<Value, DomainReportError> {
    let object = request.as_object().ok_or(DomainReportError::NotObject)?;
    let group_id = required_text(object, "group_id")?;
    let domains = required_text_set(object, "domains", MAX_DOMAIN_REPORT_DOMAINS)?;
    if domains.is_empty() {
        return Err(DomainReportError::InvalidField("domains".into()));
    }
    let subject_id = required_text(object, "subject_id")?;
    let source_tool = required_text(object, "source_tool")?;
    let report = object
        .get("report")
        .filter(|value| value.is_object())
        .cloned()
        .ok_or_else(|| DomainReportError::InvalidField("report".into()))?;
    let claim_posture = claim_posture(object.get("claim_posture"))?;
    let parent_digests = digest_set(object, "parent_digests", MAX_DOMAIN_REPORT_PARENTS)?;

    let result = json!({
        "schema": DOMAIN_REPORT_SCHEMA_VERSION,
        "workflow": DOMAIN_REPORT_PROJECT_WORKFLOW,
        "group_id": group_id,
        "domains": domains,
        "subject_id": subject_id,
        "source_tool": source_tool,
        "report": report,
        "claim_posture": claim_posture,
        "parent_digests": parent_digests,
        "readiness_claimed": false,
        "execution": "not_started",
        "guarantees": [
            "the projection preserves caller-supplied report JSON under an explicit capability group and source tool",
            "claim posture and limitations remain adjacent to the report payload",
            "the projection does not execute the source tool or infer scientific truth"
        ],
        "does_not_claim": [
            "report structure proves scientific, clinical, causal, publication, or release validity",
            "artifact indexing proves external provenance or completeness"
        ]
    });
    ensure_size(&result)?;
    validate_domain_report(&result)?;
    Ok(result)
}

/// Validate a previously projected domain report before it is indexed or restored.
pub fn validate_domain_report(report: &Value) -> Result<(), DomainReportError> {
    let object = report.as_object().ok_or(DomainReportError::NotObject)?;
    exact_text(object, "schema", DOMAIN_REPORT_SCHEMA_VERSION)?;
    exact_text(object, "workflow", DOMAIN_REPORT_PROJECT_WORKFLOW)?;
    required_text(object, "group_id")?;
    let domains = required_text_set(object, "domains", MAX_DOMAIN_REPORT_DOMAINS)?;
    if domains.is_empty() {
        return Err(DomainReportError::InvalidField("domains".into()));
    }
    required_text(object, "subject_id")?;
    required_text(object, "source_tool")?;
    if !object.get("report").is_some_and(Value::is_object) {
        return Err(DomainReportError::InvalidField("report".into()));
    }
    let _ = claim_posture(object.get("claim_posture"))?;
    let _ = digest_set(object, "parent_digests", MAX_DOMAIN_REPORT_PARENTS)?;
    if object.get("readiness_claimed") != Some(&Value::Bool(false)) {
        return Err(DomainReportError::InvalidField("readiness_claimed".into()));
    }
    exact_text(object, "execution", "not_started")?;
    required_text_set(object, "does_not_claim", MAX_DOMAIN_REPORT_NON_CLAIMS)?;
    if object
        .get("guarantees")
        .and_then(Value::as_array)
        .is_none_or(|values| values.is_empty())
    {
        return Err(DomainReportError::InvalidField("guarantees".into()));
    }
    ensure_size(report)
}

fn claim_posture(value: Option<&Value>) -> Result<Value, DomainReportError> {
    let object = value
        .and_then(Value::as_object)
        .ok_or_else(|| DomainReportError::InvalidField("claim_posture".into()))?;
    let status = required_text(object, "status")?;
    if !CLAIM_STATUSES.contains(&status.as_str()) {
        return Err(DomainReportError::InvalidClaimStatus(status));
    }
    let does_not_claim = required_text_set(object, "does_not_claim", MAX_DOMAIN_REPORT_NON_CLAIMS)?;
    if does_not_claim.is_empty() {
        return Err(DomainReportError::InvalidField(
            "claim_posture.does_not_claim".into(),
        ));
    }
    let limitations = optional_text_set(object, "limitations", MAX_DOMAIN_REPORT_LIMITATIONS)?;
    Ok(json!({
        "status": status,
        "does_not_claim": does_not_claim,
        "limitations": limitations
    }))
}

fn required_text(object: &Map<String, Value>, field: &str) -> Result<String, DomainReportError> {
    let value = object
        .get(field)
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| DomainReportError::InvalidField(field.into()))?;
    if value.len() > MAX_DOMAIN_REPORT_TEXT_BYTES {
        return Err(DomainReportError::TextTooLarge {
            field: field.into(),
            maximum: MAX_DOMAIN_REPORT_TEXT_BYTES,
        });
    }
    Ok(value.to_string())
}

fn exact_text(
    object: &Map<String, Value>,
    field: &str,
    expected: &str,
) -> Result<(), DomainReportError> {
    if object.get(field).and_then(Value::as_str) != Some(expected) {
        return Err(DomainReportError::InvalidField(field.into()));
    }
    Ok(())
}

fn required_text_set(
    object: &Map<String, Value>,
    field: &str,
    maximum: usize,
) -> Result<Vec<String>, DomainReportError> {
    let values = object
        .get(field)
        .and_then(Value::as_array)
        .ok_or_else(|| DomainReportError::InvalidField(field.into()))?;
    text_set(values, field, maximum)
}

fn optional_text_set(
    object: &Map<String, Value>,
    field: &str,
    maximum: usize,
) -> Result<Vec<String>, DomainReportError> {
    match object.get(field) {
        None | Some(Value::Null) => Ok(Vec::new()),
        Some(value) => value
            .as_array()
            .ok_or_else(|| DomainReportError::InvalidField(field.into()))
            .and_then(|values| text_set(values, field, maximum)),
    }
}

fn text_set(
    values: &[Value],
    field: &str,
    maximum: usize,
) -> Result<Vec<String>, DomainReportError> {
    if values.len() > maximum {
        return Err(DomainReportError::TooManyItems {
            field: field.into(),
            maximum,
        });
    }
    let mut result = BTreeSet::new();
    for value in values {
        let text = value
            .as_str()
            .filter(|text| !text.trim().is_empty())
            .ok_or_else(|| DomainReportError::InvalidField(field.into()))?;
        if text.len() > MAX_DOMAIN_REPORT_TEXT_BYTES {
            return Err(DomainReportError::TextTooLarge {
                field: field.into(),
                maximum: MAX_DOMAIN_REPORT_TEXT_BYTES,
            });
        }
        result.insert(text.to_string());
    }
    Ok(result.into_iter().collect())
}

fn digest_set(
    object: &Map<String, Value>,
    field: &str,
    maximum: usize,
) -> Result<Vec<String>, DomainReportError> {
    let Some(value) = object.get(field) else {
        return Ok(Vec::new());
    };
    let values = value
        .as_array()
        .ok_or_else(|| DomainReportError::InvalidField(field.into()))?;
    let values = text_set(values, field, maximum)?;
    for value in &values {
        ContentHash::parse(value.clone()).map_err(|_| DomainReportError::InvalidDigest {
            field: field.into(),
        })?;
    }
    Ok(values)
}

fn ensure_size(value: &Value) -> Result<(), DomainReportError> {
    let actual = serde_json::to_vec(value)
        .map_err(|error| DomainReportError::Canonicalisation(error.to_string()))?
        .len();
    if actual > MAX_DOMAIN_REPORT_BYTES {
        return Err(DomainReportError::TooLarge {
            actual,
            maximum: MAX_DOMAIN_REPORT_BYTES,
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
            "domains": ["biology", "biology"],
            "subject_id": "subject-1",
            "source_tool": "modality_catalog",
            "report": {"observations": ["x"]},
            "claim_posture": {
                "status": "review_required",
                "does_not_claim": ["clinical validity"],
                "limitations": ["caller supplied"]
            }
        })
    }

    #[test]
    fn projection_is_canonical_and_validates() {
        let first = project_domain_report(&request()).unwrap();
        let second = project_domain_report(&request()).unwrap();
        assert_eq!(first, second);
        assert_eq!(first["readiness_claimed"], false);
        assert_eq!(first["claim_posture"]["status"], "review_required");
        validate_domain_report(&first).unwrap();
    }

    #[test]
    fn rejects_empty_non_claims_and_bad_parent_digest() {
        let mut empty = request();
        empty["claim_posture"]["does_not_claim"] = json!([]);
        assert!(matches!(
            project_domain_report(&empty),
            Err(DomainReportError::InvalidField(_))
        ));

        let mut bad_parent = request();
        bad_parent["parent_digests"] = json!(["not-a-digest"]);
        assert!(matches!(
            project_domain_report(&bad_parent),
            Err(DomainReportError::InvalidDigest { .. })
        ));
    }

    #[test]
    fn rejects_invalid_claim_status() {
        let mut invalid = request();
        invalid["claim_posture"]["status"] = json!("validated");
        assert!(matches!(
            project_domain_report(&invalid),
            Err(DomainReportError::InvalidClaimStatus(_))
        ));
    }
}
