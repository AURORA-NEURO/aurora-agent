//! Deterministic planning for external evidence sources.
//!
//! A source plan is the safe seam between an agent's domain request and a later connector. It
//! records what kind of source a caller intends to use, where it claims the source is, which
//! retrieval policy would bound a future connector, and which capability group/domain scope owns
//! the plan. It never fetches bytes, resolves credentials, follows redirects, or turns a locator
//! into provenance. A later raw-intake artifact can parent this plan after a separately controlled
//! connector has produced a response.

use bioprism_ids::ContentHash;
use serde_json::{json, Map, Value};
use std::collections::BTreeSet;
use thiserror::Error;

pub const DOMAIN_EVIDENCE_SOURCE_PLAN_SCHEMA_VERSION: &str =
    "bioprism-devplat-domain-evidence-source-plan/0.1";
pub const DOMAIN_EVIDENCE_SOURCE_PLAN_WORKFLOW: &str = "domain_evidence_source_plan";
pub const MAX_DOMAIN_EVIDENCE_SOURCE_PLAN_BYTES: usize = 1024 * 1024;
pub const MAX_DOMAIN_EVIDENCE_SOURCE_PLAN_DOMAINS: usize = 64;
pub const MAX_DOMAIN_EVIDENCE_SOURCE_PLAN_PARENTS: usize = 128;
pub const MAX_DOMAIN_EVIDENCE_SOURCE_PLAN_TEXT_BYTES: usize = 2048;
pub const MAX_DOMAIN_EVIDENCE_SOURCE_PLAN_NON_CLAIMS: usize = 64;
pub const MAX_DOMAIN_EVIDENCE_SOURCE_PLAN_BYTES_LIMIT: u64 = 64 * 1024 * 1024;
pub const MAX_DOMAIN_EVIDENCE_SOURCE_PLAN_ALLOWED_HOSTS: usize = 32;
pub const MAX_DOMAIN_EVIDENCE_SOURCE_PLAN_TIMEOUT_MS: u64 = 30_000;

const CONNECTOR_KINDS: &[&str] = &[
    "literature",
    "clinical_trial",
    "fhir",
    "object_store",
    "file",
    "provider_api",
    "generic_http",
];
const LOCATOR_KINDS: &[&str] = &["uri", "path", "opaque"];
const RETRIEVAL_MODES: &[&str] = &["reference_only", "metadata_only", "content"];
const NETWORK_MODES: &[&str] = &["disabled", "caller_managed", "enabled"];
const CACHE_MODES: &[&str] = &["no_cache", "content_addressed"];

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum DomainEvidenceSourcePlanError {
    #[error("domain evidence source plan must be a JSON object")]
    NotObject,
    #[error("domain evidence source plan field {0} is missing or invalid")]
    InvalidField(String),
    #[error("domain evidence source plan field {field} exceeds the {maximum}-byte bound")]
    TextTooLarge { field: String, maximum: usize },
    #[error("domain evidence source plan field {field} exceeds the {maximum}-item bound")]
    TooManyItems { field: String, maximum: usize },
    #[error("domain evidence source plan is {actual} bytes, above the {maximum}-byte bound")]
    TooLarge { actual: usize, maximum: usize },
    #[error(
        "domain evidence source plan digest {field} is not a lowercase 64-character SHA-256 digest"
    )]
    InvalidDigest { field: String },
    #[error("domain evidence source plan field {field} must be one of: {allowed}")]
    InvalidChoice { field: String, allowed: String },
    #[error("domain evidence source plan could not be canonicalised: {0}")]
    Canonicalisation(String),
}

/// Normalize and digest one caller-owned external evidence source declaration.
pub fn plan_domain_evidence_source(
    request: &Value,
) -> Result<Value, DomainEvidenceSourcePlanError> {
    let object = request
        .as_object()
        .ok_or(DomainEvidenceSourcePlanError::NotObject)?;
    let group_id = required_text(object, "group_id")?;
    let domains = required_text_set(object, "domains", MAX_DOMAIN_EVIDENCE_SOURCE_PLAN_DOMAINS)?;
    if domains.is_empty() {
        return Err(DomainEvidenceSourcePlanError::InvalidField(
            "domains".into(),
        ));
    }
    let subject_id = required_text(object, "subject_id")?;
    let connector_kind = choice(object, "connector_kind", CONNECTOR_KINDS)?;
    let locator_kind = choice(object, "locator_kind", LOCATOR_KINDS)?;
    let locator = required_text(object, "locator")?;
    validate_locator(&locator)?;
    let retrieval_mode = choice(object, "retrieval_mode", RETRIEVAL_MODES)?;
    let expected_content_digest = optional_digest(object, "expected_content_digest")?;
    let parent_digests = digest_set(
        object,
        "parent_digests",
        MAX_DOMAIN_EVIDENCE_SOURCE_PLAN_PARENTS,
    )?;
    let source_tool = optional_text(object, "source_tool")?;
    let does_not_claim = required_text_set(
        object,
        "does_not_claim",
        MAX_DOMAIN_EVIDENCE_SOURCE_PLAN_NON_CLAIMS,
    )?;
    if does_not_claim.is_empty() {
        return Err(DomainEvidenceSourcePlanError::InvalidField(
            "does_not_claim".into(),
        ));
    }
    let retrieval_policy = normalize_policy(object.get("retrieval_policy"))?;

    let mut result = json!({
        "schema": DOMAIN_EVIDENCE_SOURCE_PLAN_SCHEMA_VERSION,
        "workflow": DOMAIN_EVIDENCE_SOURCE_PLAN_WORKFLOW,
        "group_id": group_id,
        "domains": domains,
        "subject_id": subject_id,
        "source_tool": source_tool,
        "connector_kind": connector_kind,
        "locator_kind": locator_kind,
        "locator": locator,
        "retrieval_mode": retrieval_mode,
        "expected_content_digest": expected_content_digest,
        "parent_digests": parent_digests,
        "retrieval_policy": retrieval_policy,
        "plan_digest": Value::Null,
        "readiness_claimed": false,
        "execution": "not_started",
        "retrieval_status": "not_started",
        "guarantees": [
            "the source locator, connector kind, retrieval mode, and policy are retained under an explicit domain scope",
            "embedded credentials, malformed digests, duplicate labels, and unbounded policy values are refused",
            "the plan is deterministic and can be used as a parent identity for later caller-controlled intake"
        ],
        "does_not_claim": does_not_claim,
    });
    let digest = digest_without_field(&result, "plan_digest")?;
    result["plan_digest"] = json!(digest);
    ensure_size(&result)?;
    validate_domain_evidence_source_plan(&result)?;
    Ok(result)
}

/// Validate a source plan before artifact registration or restart restoration.
pub fn validate_domain_evidence_source_plan(
    plan: &Value,
) -> Result<(), DomainEvidenceSourcePlanError> {
    let object = plan
        .as_object()
        .ok_or(DomainEvidenceSourcePlanError::NotObject)?;
    exact_text(object, "schema", DOMAIN_EVIDENCE_SOURCE_PLAN_SCHEMA_VERSION)?;
    exact_text(object, "workflow", DOMAIN_EVIDENCE_SOURCE_PLAN_WORKFLOW)?;
    required_text(object, "group_id")?;
    let domains = required_text_set(object, "domains", MAX_DOMAIN_EVIDENCE_SOURCE_PLAN_DOMAINS)?;
    if domains.is_empty() {
        return Err(DomainEvidenceSourcePlanError::InvalidField(
            "domains".into(),
        ));
    }
    required_text(object, "subject_id")?;
    if let Some(source_tool) = object.get("source_tool") {
        if !source_tool.is_null() {
            optional_text(object, "source_tool")?;
        }
    }
    choice(object, "connector_kind", CONNECTOR_KINDS)?;
    choice(object, "locator_kind", LOCATOR_KINDS)?;
    let locator = required_text(object, "locator")?;
    validate_locator(&locator)?;
    choice(object, "retrieval_mode", RETRIEVAL_MODES)?;
    let _ = optional_digest(object, "expected_content_digest")?;
    let _ = digest_set(
        object,
        "parent_digests",
        MAX_DOMAIN_EVIDENCE_SOURCE_PLAN_PARENTS,
    )?;
    normalize_policy(object.get("retrieval_policy"))?;
    let _ = required_text_set(
        object,
        "does_not_claim",
        MAX_DOMAIN_EVIDENCE_SOURCE_PLAN_NON_CLAIMS,
    )?;
    if object.get("readiness_claimed") != Some(&Value::Bool(false)) {
        return Err(DomainEvidenceSourcePlanError::InvalidField(
            "readiness_claimed".into(),
        ));
    }
    exact_text(object, "execution", "not_started")?;
    exact_text(object, "retrieval_status", "not_started")?;
    let claimed_digest = required_digest(object, "plan_digest")?;
    let recomputed_digest = digest_without_field(plan, "plan_digest")?;
    if claimed_digest != recomputed_digest {
        return Err(DomainEvidenceSourcePlanError::InvalidField(
            "plan_digest does not match canonical plan".into(),
        ));
    }
    if object
        .get("guarantees")
        .and_then(Value::as_array)
        .is_none_or(|values| values.is_empty())
    {
        return Err(DomainEvidenceSourcePlanError::InvalidField(
            "guarantees".into(),
        ));
    }
    ensure_size(plan)
}

fn normalize_policy(value: Option<&Value>) -> Result<Value, DomainEvidenceSourcePlanError> {
    let object = match value {
        None | Some(Value::Null) => Map::new(),
        Some(value) => value.as_object().cloned().ok_or_else(|| {
            DomainEvidenceSourcePlanError::InvalidField("retrieval_policy".into())
        })?,
    };
    let network = object
        .get("network")
        .and_then(Value::as_str)
        .unwrap_or("caller_managed");
    if !NETWORK_MODES.contains(&network) {
        return Err(DomainEvidenceSourcePlanError::InvalidChoice {
            field: "retrieval_policy.network".into(),
            allowed: NETWORK_MODES.join(", "),
        });
    }
    let max_bytes = object
        .get("max_bytes")
        .and_then(Value::as_u64)
        .unwrap_or(2 * 1024 * 1024);
    if !(1..=MAX_DOMAIN_EVIDENCE_SOURCE_PLAN_BYTES_LIMIT).contains(&max_bytes) {
        return Err(DomainEvidenceSourcePlanError::InvalidField(
            "retrieval_policy.max_bytes".into(),
        ));
    }
    let cache = object
        .get("cache")
        .and_then(Value::as_str)
        .unwrap_or("content_addressed");
    if !CACHE_MODES.contains(&cache) {
        return Err(DomainEvidenceSourcePlanError::InvalidChoice {
            field: "retrieval_policy.cache".into(),
            allowed: CACHE_MODES.join(", "),
        });
    }
    let timeout_ms = object
        .get("timeout_ms")
        .and_then(Value::as_u64)
        .unwrap_or(5_000);
    if !(1..=MAX_DOMAIN_EVIDENCE_SOURCE_PLAN_TIMEOUT_MS).contains(&timeout_ms) {
        return Err(DomainEvidenceSourcePlanError::InvalidField(
            "retrieval_policy.timeout_ms".into(),
        ));
    }
    let allowed_hosts = text_set_value(
        object.get("allowed_hosts"),
        "retrieval_policy.allowed_hosts",
        MAX_DOMAIN_EVIDENCE_SOURCE_PLAN_ALLOWED_HOSTS,
    )?;
    if network == "enabled" && allowed_hosts.is_empty() {
        return Err(DomainEvidenceSourcePlanError::InvalidField(
            "retrieval_policy.allowed_hosts is required when network is enabled".into(),
        ));
    }
    Ok(json!({
        "network": network,
        "max_bytes": max_bytes,
        "cache": cache,
        "timeout_ms": timeout_ms,
        "allowed_hosts": allowed_hosts,
        "credentials": "caller_managed_not_supplied"
    }))
}

fn text_set_value(
    value: Option<&Value>,
    field: &str,
    maximum: usize,
) -> Result<Vec<String>, DomainEvidenceSourcePlanError> {
    let Some(value) = value else {
        return Ok(Vec::new());
    };
    let values = value
        .as_array()
        .ok_or_else(|| DomainEvidenceSourcePlanError::InvalidField(field.into()))?;
    if values.len() > maximum {
        return Err(DomainEvidenceSourcePlanError::TooManyItems {
            field: field.into(),
            maximum,
        });
    }
    let mut result = BTreeSet::new();
    for value in values {
        let host = value
            .as_str()
            .map(str::trim)
            .filter(|host| !host.is_empty())
            .ok_or_else(|| DomainEvidenceSourcePlanError::InvalidField(field.into()))?;
        if host.len() > MAX_DOMAIN_EVIDENCE_SOURCE_PLAN_TEXT_BYTES
            || host.contains(['\r', '\n', '/', '?', '#', '@', ':', ' '])
        {
            return Err(DomainEvidenceSourcePlanError::InvalidField(field.into()));
        }
        result.insert(host.trim_end_matches('.').to_ascii_lowercase());
    }
    Ok(result.into_iter().collect())
}

fn required_text(
    object: &Map<String, Value>,
    field: &str,
) -> Result<String, DomainEvidenceSourcePlanError> {
    let value = object
        .get(field)
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| DomainEvidenceSourcePlanError::InvalidField(field.into()))?;
    if value.len() > MAX_DOMAIN_EVIDENCE_SOURCE_PLAN_TEXT_BYTES {
        return Err(DomainEvidenceSourcePlanError::TextTooLarge {
            field: field.into(),
            maximum: MAX_DOMAIN_EVIDENCE_SOURCE_PLAN_TEXT_BYTES,
        });
    }
    Ok(value.to_string())
}

fn optional_text(
    object: &Map<String, Value>,
    field: &str,
) -> Result<Option<String>, DomainEvidenceSourcePlanError> {
    match object.get(field) {
        None | Some(Value::Null) => Ok(None),
        Some(_) => required_text(object, field).map(Some),
    }
}

fn exact_text(
    object: &Map<String, Value>,
    field: &str,
    expected: &str,
) -> Result<(), DomainEvidenceSourcePlanError> {
    if object.get(field).and_then(Value::as_str) != Some(expected) {
        return Err(DomainEvidenceSourcePlanError::InvalidField(field.into()));
    }
    Ok(())
}

fn validate_locator(locator: &str) -> Result<(), DomainEvidenceSourcePlanError> {
    if locator.contains(['\r', '\n']) {
        return Err(DomainEvidenceSourcePlanError::InvalidField(
            "locator contains a control line break".into(),
        ));
    }
    if let Some(scheme_end) = locator.find("://") {
        let authority = &locator[scheme_end + 3..];
        let authority_end = authority.find(['/', '?', '#']).unwrap_or(authority.len());
        if authority[..authority_end].contains('@') {
            return Err(DomainEvidenceSourcePlanError::InvalidField(
                "locator must not contain embedded credentials".into(),
            ));
        }
    }
    Ok(())
}

fn choice(
    object: &Map<String, Value>,
    field: &str,
    allowed: &[&str],
) -> Result<String, DomainEvidenceSourcePlanError> {
    let value = required_text(object, field)?;
    if !allowed.contains(&value.as_str()) {
        return Err(DomainEvidenceSourcePlanError::InvalidChoice {
            field: field.into(),
            allowed: allowed.join(", "),
        });
    }
    Ok(value)
}

fn required_text_set(
    object: &Map<String, Value>,
    field: &str,
    maximum: usize,
) -> Result<Vec<String>, DomainEvidenceSourcePlanError> {
    let values = object
        .get(field)
        .and_then(Value::as_array)
        .ok_or_else(|| DomainEvidenceSourcePlanError::InvalidField(field.into()))?;
    if values.len() > maximum {
        return Err(DomainEvidenceSourcePlanError::TooManyItems {
            field: field.into(),
            maximum,
        });
    }
    let mut result = BTreeSet::new();
    for value in values {
        let text = value
            .as_str()
            .filter(|text| !text.trim().is_empty())
            .ok_or_else(|| DomainEvidenceSourcePlanError::InvalidField(field.into()))?;
        if text.len() > MAX_DOMAIN_EVIDENCE_SOURCE_PLAN_TEXT_BYTES {
            return Err(DomainEvidenceSourcePlanError::TextTooLarge {
                field: field.into(),
                maximum: MAX_DOMAIN_EVIDENCE_SOURCE_PLAN_TEXT_BYTES,
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
) -> Result<Vec<String>, DomainEvidenceSourcePlanError> {
    let Some(value) = object.get(field) else {
        return Ok(Vec::new());
    };
    let values = value
        .as_array()
        .ok_or_else(|| DomainEvidenceSourcePlanError::InvalidField(field.into()))?;
    if values.len() > maximum {
        return Err(DomainEvidenceSourcePlanError::TooManyItems {
            field: field.into(),
            maximum,
        });
    }
    let mut result = BTreeSet::new();
    for value in values {
        let digest = value
            .as_str()
            .filter(|value| !value.trim().is_empty())
            .ok_or_else(|| DomainEvidenceSourcePlanError::InvalidField(field.into()))?;
        ContentHash::parse(digest.to_string()).map_err(|_| {
            DomainEvidenceSourcePlanError::InvalidDigest {
                field: field.into(),
            }
        })?;
        result.insert(digest.to_string());
    }
    Ok(result.into_iter().collect())
}

fn optional_digest(
    object: &Map<String, Value>,
    field: &str,
) -> Result<Option<String>, DomainEvidenceSourcePlanError> {
    match object.get(field) {
        None | Some(Value::Null) => Ok(None),
        Some(value) => {
            let digest = value
                .as_str()
                .filter(|value| !value.trim().is_empty())
                .ok_or_else(|| DomainEvidenceSourcePlanError::InvalidField(field.into()))?;
            ContentHash::parse(digest.to_string()).map_err(|_| {
                DomainEvidenceSourcePlanError::InvalidDigest {
                    field: field.into(),
                }
            })?;
            Ok(Some(digest.to_string()))
        }
    }
}

fn required_digest(
    object: &Map<String, Value>,
    field: &str,
) -> Result<String, DomainEvidenceSourcePlanError> {
    optional_digest(object, field)?
        .ok_or_else(|| DomainEvidenceSourcePlanError::InvalidField(field.into()))
}

fn digest_without_field(
    value: &Value,
    field: &str,
) -> Result<String, DomainEvidenceSourcePlanError> {
    let mut without = value.clone();
    without
        .as_object_mut()
        .ok_or(DomainEvidenceSourcePlanError::NotObject)?
        .remove(field);
    ContentHash::of_value(&without)
        .map(|digest| digest.to_string())
        .map_err(|error| DomainEvidenceSourcePlanError::Canonicalisation(error.to_string()))
}

fn ensure_size(value: &Value) -> Result<(), DomainEvidenceSourcePlanError> {
    let actual = serde_json::to_vec(value)
        .map_err(|error| DomainEvidenceSourcePlanError::Canonicalisation(error.to_string()))?
        .len();
    if actual > MAX_DOMAIN_EVIDENCE_SOURCE_PLAN_BYTES {
        return Err(DomainEvidenceSourcePlanError::TooLarge {
            actual,
            maximum: MAX_DOMAIN_EVIDENCE_SOURCE_PLAN_BYTES,
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
            "subject_id": "source-subject",
            "source_tool": "modality_catalog",
            "connector_kind": "literature",
            "locator_kind": "uri",
            "locator": "https://example.org/article/1",
            "retrieval_mode": "metadata_only",
            "expected_content_digest": "a".repeat(64),
            "retrieval_policy": {"network": "caller_managed", "max_bytes": 4096, "cache": "content_addressed"},
            "does_not_claim": ["retrieval occurred", "source is true"]
        })
    }

    #[test]
    fn plan_is_deterministic_and_digest_verified() {
        let first = plan_domain_evidence_source(&request()).unwrap();
        let second = plan_domain_evidence_source(&request()).unwrap();
        assert_eq!(first, second);
        assert_eq!(first["retrieval_status"], "not_started");
        validate_domain_evidence_source_plan(&first).unwrap();
    }

    #[test]
    fn refuses_embedded_credentials_and_invalid_policy() {
        let mut credentials = request();
        credentials["locator"] = json!("https://user:pass@example.org/article/1");
        assert!(matches!(
            plan_domain_evidence_source(&credentials),
            Err(DomainEvidenceSourcePlanError::InvalidField(_))
        ));
        let mut policy = request();
        policy["retrieval_policy"]["max_bytes"] =
            json!(MAX_DOMAIN_EVIDENCE_SOURCE_PLAN_BYTES_LIMIT + 1);
        assert!(matches!(
            plan_domain_evidence_source(&policy),
            Err(DomainEvidenceSourcePlanError::InvalidField(_))
        ));
    }

    #[test]
    fn detects_tampered_plan_digest() {
        let mut plan = plan_domain_evidence_source(&request()).unwrap();
        plan["locator"] = json!("https://example.org/article/2");
        assert!(matches!(
            validate_domain_evidence_source_plan(&plan),
            Err(DomainEvidenceSourcePlanError::InvalidField(_))
        ));
    }
}
