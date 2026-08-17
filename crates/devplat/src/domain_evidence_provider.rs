//! Normalize caller-supplied provider payloads into the domain-evidence intake envelope.
//!
//! The source connector kernel deliberately does not contact literature indexes, trial
//! registries, FHIR servers, object stores, or arbitrary provider APIs. Those integrations are
//! caller-managed, but callers still need one exact handoff into the same digest-bound intake
//! and coverage machinery. This module owns that structural handoff without authenticating a
//! provider, interpreting domain values, or inferring an outcome from payload shape.

use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use thiserror::Error;

pub const DOMAIN_EVIDENCE_PROVIDER_NORMALIZATION_SCHEMA: &str =
    "bioprism-devplat-domain-evidence-provider-normalization/0.1";
pub const DOMAIN_EVIDENCE_PROVIDER_NORMALIZATION_WORKFLOW: &str =
    "domain_evidence_provider_normalize";
pub const MAX_DOMAIN_EVIDENCE_PROVIDER_BYTES: usize = 20_000_000;
pub const MAX_DOMAIN_EVIDENCE_PROVIDER_TEXT_BYTES: usize = 512;
pub const MAX_DOMAIN_EVIDENCE_PROVIDER_DOMAINS: usize = 64;
pub const MAX_DOMAIN_EVIDENCE_PROVIDER_PARENTS: usize = 128;

const CONNECTOR_KINDS: &[&str] = &[
    "literature",
    "clinical_trial",
    "fhir",
    "object_store",
    "provider_api",
];
const OUTCOMES: &[&str] = &["observed", "partial", "refused", "error", "unknown"];

fn default_outcome() -> String {
    "unknown".into()
}

fn default_claim_posture() -> Value {
    json!({
        "status": "review_required",
        "does_not_claim": [
            "provider authenticity",
            "scientific or clinical validity",
            "provenance completeness",
            "execution or external effect"
        ]
    })
}

/// Caller-supplied provider payload plus the exact domain scope it purports to cover.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DomainEvidenceProviderNormalizationRequest {
    pub group_id: String,
    pub domains: Vec<String>,
    pub subject_id: String,
    pub source_tool: String,
    pub connector_kind: String,
    pub provider: String,
    pub payload: Value,
    #[serde(default)]
    pub request: Option<Value>,
    #[serde(default = "default_outcome")]
    pub outcome: String,
    #[serde(default = "default_claim_posture")]
    pub claim_posture: Value,
    #[serde(default)]
    pub parent_digests: Vec<String>,
    #[serde(default)]
    pub source_plan_digest: Option<String>,
}

/// Canonical provider envelope and the arguments ready for `domain_evidence_intake`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DomainEvidenceProviderNormalization {
    pub schema: String,
    pub workflow: String,
    pub group_id: String,
    pub domains: Vec<String>,
    pub subject_id: String,
    pub source_tool: String,
    pub connector_kind: String,
    pub provider: String,
    pub outcome: String,
    pub payload_digest: String,
    pub request_digest: Option<String>,
    pub response: Value,
    /// Internal composition input; never duplicate the full intake envelope in the public result.
    #[serde(skip)]
    pub intake_arguments: Value,
    pub guarantees: Vec<String>,
    pub limitations: Vec<String>,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum DomainEvidenceProviderNormalizationError {
    #[error("{field} must be a non-empty value no longer than {MAX_DOMAIN_EVIDENCE_PROVIDER_TEXT_BYTES} bytes")]
    InvalidText { field: &'static str },
    #[error("connector_kind must be one of: {0}")]
    UnsupportedConnector(String),
    #[error("domains must contain between 1 and {MAX_DOMAIN_EVIDENCE_PROVIDER_DOMAINS} values")]
    InvalidDomains,
    #[error("outcome must be one of: {0}")]
    InvalidOutcome(String),
    #[error("claim_posture must be an object")]
    InvalidClaimPosture,
    #[error("claim_posture.status must be observed, derived, review_required, refused, or not_applicable")]
    InvalidClaimStatus,
    #[error("claim_posture.does_not_claim must contain at least one string")]
    MissingNonClaims,
    #[error("too many parent digests")]
    TooManyParents,
    #[error("{field} is not a valid lowercase SHA-256 digest: {value}")]
    InvalidDigest { field: &'static str, value: String },
    #[error("payload must be an object or array")]
    InvalidPayload,
    #[error("cannot canonicalize provider payload: {0}")]
    Canonical(String),
    #[error(
        "provider envelope exceeds the {MAX_DOMAIN_EVIDENCE_PROVIDER_BYTES}-byte safety bound"
    )]
    TooLarge,
}

fn bounded_text(
    field: &'static str,
    value: &str,
) -> Result<String, DomainEvidenceProviderNormalizationError> {
    if value.trim().is_empty()
        || value.len() > MAX_DOMAIN_EVIDENCE_PROVIDER_TEXT_BYTES
        || value.chars().any(char::is_control)
    {
        return Err(DomainEvidenceProviderNormalizationError::InvalidText { field });
    }
    Ok(value.to_owned())
}

fn digest(
    field: &'static str,
    value: &str,
) -> Result<String, DomainEvidenceProviderNormalizationError> {
    let value = bounded_text(field, value)?;
    if value.len() != 64
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    {
        return Err(DomainEvidenceProviderNormalizationError::InvalidDigest { field, value });
    }
    ContentHash::parse(value.clone()).map_err(|_| {
        DomainEvidenceProviderNormalizationError::InvalidDigest {
            field,
            value: value.clone(),
        }
    })?;
    Ok(value)
}

fn canonical_digest(value: &Value) -> Result<String, DomainEvidenceProviderNormalizationError> {
    ContentHash::of_value(value)
        .map(|digest| digest.to_string())
        .map_err(|error| DomainEvidenceProviderNormalizationError::Canonical(error.to_string()))
}

fn validate_claim_posture(value: &Value) -> Result<(), DomainEvidenceProviderNormalizationError> {
    let object = value
        .as_object()
        .ok_or(DomainEvidenceProviderNormalizationError::InvalidClaimPosture)?;
    let status = object
        .get("status")
        .and_then(Value::as_str)
        .ok_or(DomainEvidenceProviderNormalizationError::InvalidClaimStatus)?;
    if ![
        "observed",
        "derived",
        "review_required",
        "refused",
        "not_applicable",
    ]
    .contains(&status)
    {
        return Err(DomainEvidenceProviderNormalizationError::InvalidClaimStatus);
    }
    let non_claims = object
        .get("does_not_claim")
        .and_then(Value::as_array)
        .ok_or(DomainEvidenceProviderNormalizationError::MissingNonClaims)?;
    if non_claims.is_empty()
        || non_claims
            .iter()
            .any(|value| value.as_str().is_none_or(|text| text.trim().is_empty()))
    {
        return Err(DomainEvidenceProviderNormalizationError::MissingNonClaims);
    }
    Ok(())
}

fn ensure_size(value: &Value) -> Result<(), DomainEvidenceProviderNormalizationError> {
    let bytes = serde_json::to_vec(value)
        .map_err(|error| DomainEvidenceProviderNormalizationError::Canonical(error.to_string()))?;
    if bytes.len() > MAX_DOMAIN_EVIDENCE_PROVIDER_BYTES {
        return Err(DomainEvidenceProviderNormalizationError::TooLarge);
    }
    Ok(())
}

/// Normalize one caller-owned provider response; no network or provider call occurs here.
pub fn normalize_domain_evidence_provider(
    request: &DomainEvidenceProviderNormalizationRequest,
) -> Result<DomainEvidenceProviderNormalization, DomainEvidenceProviderNormalizationError> {
    let group_id = bounded_text("group_id", &request.group_id)?;
    let subject_id = bounded_text("subject_id", &request.subject_id)?;
    let source_tool = bounded_text("source_tool", &request.source_tool)?;
    let provider = bounded_text("provider", &request.provider)?;
    if !CONNECTOR_KINDS.contains(&request.connector_kind.as_str()) {
        return Err(
            DomainEvidenceProviderNormalizationError::UnsupportedConnector(
                request.connector_kind.clone(),
            ),
        );
    }
    let connector_kind = bounded_text("connector_kind", &request.connector_kind)?;
    if request.domains.is_empty() || request.domains.len() > MAX_DOMAIN_EVIDENCE_PROVIDER_DOMAINS {
        return Err(DomainEvidenceProviderNormalizationError::InvalidDomains);
    }
    let domains = request
        .domains
        .iter()
        .map(|domain| bounded_text("domain", domain))
        .collect::<Result<Vec<_>, _>>()?;
    if !OUTCOMES.contains(&request.outcome.as_str()) {
        return Err(DomainEvidenceProviderNormalizationError::InvalidOutcome(
            request.outcome.clone(),
        ));
    }
    validate_claim_posture(&request.claim_posture)?;
    if request.parent_digests.len() > MAX_DOMAIN_EVIDENCE_PROVIDER_PARENTS {
        return Err(DomainEvidenceProviderNormalizationError::TooManyParents);
    }
    let parent_digests = request
        .parent_digests
        .iter()
        .map(|parent| digest("parent_digest", parent))
        .collect::<Result<Vec<_>, _>>()?;
    let source_plan_digest = request
        .source_plan_digest
        .as_deref()
        .map(|value| digest("source_plan_digest", value))
        .transpose()?;
    if !request.payload.is_object() && !request.payload.is_array() {
        return Err(DomainEvidenceProviderNormalizationError::InvalidPayload);
    }
    ensure_size(&request.payload)?;
    if let Some(request_value) = request.request.as_ref() {
        ensure_size(request_value)?;
    }
    ensure_size(&request.claim_posture)?;

    let payload_digest = canonical_digest(&request.payload)?;
    let request_digest = request.request.as_ref().map(canonical_digest).transpose()?;
    let mut response = Map::new();
    response.insert("provider".into(), json!(provider));
    response.insert("connector_kind".into(), json!(connector_kind));
    response.insert("source".into(), json!("caller_supplied"));
    response.insert("authenticated".into(), Value::Bool(false));
    response.insert("payload_digest".into(), json!(payload_digest));
    response.insert("payload".into(), request.payload.clone());
    let response = Value::Object(response);
    let mut intake_arguments = Map::new();
    intake_arguments.insert("group_id".into(), json!(group_id));
    intake_arguments.insert("domains".into(), json!(domains));
    intake_arguments.insert("subject_id".into(), json!(subject_id));
    intake_arguments.insert("source_tool".into(), json!(source_tool));
    intake_arguments.insert("response".into(), response.clone());
    intake_arguments.insert("outcome".into(), json!(request.outcome));
    intake_arguments.insert("claim_posture".into(), request.claim_posture.clone());
    intake_arguments.insert("parent_digests".into(), json!(parent_digests));
    if let Some(source_plan_digest) = source_plan_digest.as_ref() {
        intake_arguments.insert("source_plan_digest".into(), json!(source_plan_digest));
    }
    if let Some(request_value) = request.request.as_ref() {
        intake_arguments.insert("request".into(), request_value.clone());
    }
    let intake_arguments = Value::Object(intake_arguments);
    ensure_size(&intake_arguments)?;
    Ok(DomainEvidenceProviderNormalization {
        schema: DOMAIN_EVIDENCE_PROVIDER_NORMALIZATION_SCHEMA.into(),
        workflow: DOMAIN_EVIDENCE_PROVIDER_NORMALIZATION_WORKFLOW.into(),
        group_id,
        domains,
        subject_id,
        source_tool,
        connector_kind,
        provider,
        outcome: request.outcome.clone(),
        payload_digest,
        request_digest,
        response,
        intake_arguments,
        guarantees: vec![
            "provider metadata and payload receive separate explicit structural identities".into(),
            "caller-supplied status remains visible and is never inferred from payload shape".into(),
            "the normalized envelope can be passed to the existing digest-bound intake boundary".into(),
        ],
        limitations: vec![
            "the route does not contact or authenticate the named provider".into(),
            "payload fields are retained structurally and are not interpreted as scientific or clinical facts".into(),
            "external signatures, terminology expansion, retrieval completeness, and provenance remain caller-managed".into(),
        ],
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base_request() -> DomainEvidenceProviderNormalizationRequest {
        DomainEvidenceProviderNormalizationRequest {
            group_id: "biological_domains".into(),
            domains: vec!["oncology".into()],
            subject_id: "subject-1".into(),
            source_tool: "literature_bind_check".into(),
            connector_kind: "literature".into(),
            provider: "pubmed".into(),
            payload: json!({"records": [{"id": "pmid:1", "title": "opaque"}]}),
            request: Some(json!({"query": "oncology"})),
            outcome: "observed".into(),
            claim_posture: default_claim_posture(),
            parent_digests: vec!["a".repeat(64)],
            source_plan_digest: Some("b".repeat(64)),
        }
    }

    #[test]
    fn normalizes_provider_payload_and_builds_intake_arguments() {
        let normalized = normalize_domain_evidence_provider(&base_request()).unwrap();
        assert_eq!(normalized.connector_kind, "literature");
        assert_eq!(normalized.outcome, "observed");
        assert_eq!(normalized.response["authenticated"], json!(false));
        assert_eq!(
            normalized.intake_arguments["source_plan_digest"],
            json!("b".repeat(64))
        );
        assert_eq!(normalized.payload_digest.len(), 64);
    }

    #[test]
    fn keeps_unknown_outcomes_explicit() {
        let mut request = base_request();
        request.outcome = "unknown".into();
        assert_eq!(
            normalize_domain_evidence_provider(&request)
                .unwrap()
                .outcome,
            "unknown"
        );
        request.request = None;
        let normalized = normalize_domain_evidence_provider(&request).unwrap();
        assert!(normalized.intake_arguments.get("request").is_none());
    }

    #[test]
    fn rejects_provider_execution_connectors_and_bad_claim_posture() {
        let mut request = base_request();
        request.connector_kind = "file".into();
        assert!(matches!(
            normalize_domain_evidence_provider(&request),
            Err(DomainEvidenceProviderNormalizationError::UnsupportedConnector(_))
        ));
        let mut request = base_request();
        request.claim_posture = json!({"status": "observed"});
        assert_eq!(
            normalize_domain_evidence_provider(&request).unwrap_err(),
            DomainEvidenceProviderNormalizationError::MissingNonClaims
        );
    }

    #[test]
    fn rejects_scalar_payloads_and_bad_digests() {
        let mut request = base_request();
        request.payload = json!("secret");
        assert_eq!(
            normalize_domain_evidence_provider(&request).unwrap_err(),
            DomainEvidenceProviderNormalizationError::InvalidPayload
        );
        let mut request = base_request();
        request.parent_digests = vec!["not-a-digest".into()];
        assert!(matches!(
            normalize_domain_evidence_provider(&request),
            Err(DomainEvidenceProviderNormalizationError::InvalidDigest { .. })
        ));
    }
}
