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
use std::collections::BTreeMap;
use thiserror::Error;

pub const DOMAIN_EVIDENCE_PROVIDER_NORMALIZATION_SCHEMA: &str =
    "bioprism-devplat-domain-evidence-provider-normalization/0.1";
pub const DOMAIN_EVIDENCE_PROVIDER_NORMALIZATION_WORKFLOW: &str =
    "domain_evidence_provider_normalize";
pub const DOMAIN_EVIDENCE_PROVIDER_SHAPE_AUDIT_SCHEMA: &str =
    "bioprism-devplat-domain-evidence-provider-shape-audit/0.1";
pub const DOMAIN_EVIDENCE_PROVIDER_REPLAY_SCHEMA: &str =
    "bioprism-devplat-domain-evidence-provider-replay/0.1";
pub const DOMAIN_EVIDENCE_PROVIDER_REPLAY_WORKFLOW: &str = "domain_evidence_provider_replay_verify";
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

/// Shape-only coverage for a bounded set of candidate fields. Values are deliberately never
/// retained here: this is a structural index, not a second provider payload.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DomainEvidenceProviderShapeCoverage {
    pub candidate_fields: Vec<String>,
    pub present_record_count: usize,
    pub missing_record_count: usize,
}

/// Deterministic structural audit of a caller-managed provider payload.
///
/// `structured` means the recognized container and all rows were structurally consumable;
/// `partial` means rows or required shape fields were missing; `refused` means a recognized
/// container had an incompatible shape; and `unclassified` means no connector-specific
/// container was recognized. None of these statuses describe scientific, clinical, or provider
/// validity.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DomainEvidenceProviderShapeAudit {
    pub schema: String,
    pub status: String,
    pub connector_kind: String,
    pub root_kind: String,
    pub recognized_container: Option<String>,
    pub record_count: usize,
    pub valid_record_count: usize,
    pub invalid_record_count: usize,
    pub identifier_coverage: DomainEvidenceProviderShapeCoverage,
    pub content_digest_coverage: Option<DomainEvidenceProviderShapeCoverage>,
    pub missing_fields: Vec<String>,
    pub warnings: Vec<String>,
    pub limitations: Vec<String>,
    pub shape_digest: String,
}

/// Re-submit one caller-managed payload and compare it with a prior retained normalization
/// without contacting the named provider. The expected digests make omissions and substitutions
/// visible instead of treating a successful parse as a replay match.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DomainEvidenceProviderReplayRequest {
    #[serde(flatten)]
    pub observation: DomainEvidenceProviderNormalizationRequest,
    pub expected_payload_digest: String,
    #[serde(default)]
    pub expected_request_digest: Option<String>,
    pub expected_shape_digest: String,
    pub expected_normalization_digest: String,
    pub expected_intake_digest: String,
}

/// Value-free replay comparison over provider, shape, normalization, and intake identities.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DomainEvidenceProviderReplayVerification {
    pub schema: String,
    pub workflow: String,
    pub replay_status: String,
    pub matched: bool,
    pub group_id: String,
    pub domains: Vec<String>,
    pub subject_id: String,
    pub source_tool: String,
    pub connector_kind: String,
    pub provider: String,
    pub expected_payload_digest: String,
    pub observed_payload_digest: String,
    pub expected_request_digest: Option<String>,
    pub observed_request_digest: Option<String>,
    pub expected_shape_digest: String,
    pub observed_shape_digest: String,
    pub expected_normalization_digest: String,
    pub observed_normalization_digest: String,
    pub expected_intake_digest: String,
    pub observed_intake_digest: String,
    pub matches: BTreeMap<String, bool>,
    pub differences: Vec<String>,
    pub shape_audit: DomainEvidenceProviderShapeAudit,
    pub replay_digest: String,
    pub guarantees: Vec<String>,
    pub limitations: Vec<String>,
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
    pub shape_audit: DomainEvidenceProviderShapeAudit,
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

fn root_kind(value: &Value) -> &'static str {
    if value.is_object() {
        "object"
    } else {
        "array"
    }
}

fn empty_coverage(fields: &[&str]) -> DomainEvidenceProviderShapeCoverage {
    DomainEvidenceProviderShapeCoverage {
        candidate_fields: fields.iter().map(|field| (*field).into()).collect(),
        present_record_count: 0,
        missing_record_count: 0,
    }
}

fn add_missing_field(audit: &mut DomainEvidenceProviderShapeAudit, field: &str) {
    if !audit
        .missing_fields
        .iter()
        .any(|candidate| candidate == field)
    {
        audit.missing_fields.push(field.into());
    }
}

fn add_warning(audit: &mut DomainEvidenceProviderShapeAudit, warning: impl Into<String>) {
    let warning = warning.into();
    if !audit.warnings.iter().any(|candidate| candidate == &warning) {
        audit.warnings.push(warning);
    }
}

fn record_has_any_field(record: &Map<String, Value>, fields: &[&str]) -> bool {
    fields
        .iter()
        .any(|field| record.get(*field).is_some_and(|value| !value.is_null()))
}

fn audit_records(
    audit: &mut DomainEvidenceProviderShapeAudit,
    records: &[Value],
    identifier_fields: &[&str],
    digest_fields: Option<&[&str]>,
) {
    audit.record_count = records.len();
    for record in records {
        let Some(record) = record.as_object() else {
            audit.invalid_record_count += 1;
            add_missing_field(audit, "record.object");
            continue;
        };
        audit.valid_record_count += 1;
        if record_has_any_field(record, identifier_fields) {
            audit.identifier_coverage.present_record_count += 1;
        } else {
            audit.identifier_coverage.missing_record_count += 1;
            add_missing_field(audit, "record.identifier");
        }
        if let Some(digest_fields) = digest_fields {
            let coverage = audit
                .content_digest_coverage
                .as_mut()
                .expect("digest coverage is initialized with digest fields");
            if record_has_any_field(record, digest_fields) {
                coverage.present_record_count += 1;
            } else {
                coverage.missing_record_count += 1;
            }
        }
    }
    if audit.record_count == 0 {
        add_warning(audit, "recognized container is empty");
    }
    if audit.invalid_record_count > 0 {
        add_warning(audit, "one or more container entries are not objects");
    }
    if audit.identifier_coverage.missing_record_count > 0 {
        add_warning(
            audit,
            "one or more object entries lack a recognized identifier field",
        );
    }
}

fn audit_array_container(
    audit: &mut DomainEvidenceProviderShapeAudit,
    container: &str,
    value: Option<&Value>,
    identifier_fields: &[&str],
    digest_fields: Option<&[&str]>,
) {
    audit.recognized_container = Some(container.into());
    match value {
        Some(Value::Array(records)) => {
            audit_records(audit, records, identifier_fields, digest_fields)
        }
        Some(_) => {
            audit.invalid_record_count = 1;
            add_missing_field(audit, &format!("{container}.array"));
            add_warning(audit, "recognized container is not an array");
        }
        None => {
            audit.invalid_record_count = 1;
            add_missing_field(audit, container);
            add_warning(audit, "recognized container is missing");
        }
    }
}

fn shape_audit_base(
    connector_kind: &str,
    payload: &Value,
    identifier_fields: &[&str],
    digest_fields: Option<&[&str]>,
) -> DomainEvidenceProviderShapeAudit {
    DomainEvidenceProviderShapeAudit {
        schema: DOMAIN_EVIDENCE_PROVIDER_SHAPE_AUDIT_SCHEMA.into(),
        status: "unclassified".into(),
        connector_kind: connector_kind.into(),
        root_kind: root_kind(payload).into(),
        recognized_container: None,
        record_count: 0,
        valid_record_count: 0,
        invalid_record_count: 0,
        identifier_coverage: empty_coverage(identifier_fields),
        content_digest_coverage: digest_fields.map(empty_coverage),
        missing_fields: Vec::new(),
        warnings: Vec::new(),
        limitations: vec![
            "the audit checks container and field presence only; it does not validate field values".into(),
            "the audit does not authenticate the provider or establish scientific, clinical, causal, regulatory, or provenance validity".into(),
            "the audit never echoes identifiers, payload values, or record contents".into(),
        ],
        shape_digest: String::new(),
    }
}

fn finish_shape_audit(
    mut audit: DomainEvidenceProviderShapeAudit,
) -> Result<DomainEvidenceProviderShapeAudit, DomainEvidenceProviderNormalizationError> {
    if audit.recognized_container.is_some() {
        audit.status = if audit.invalid_record_count > 0 && audit.valid_record_count == 0 {
            "refused".into()
        } else if audit.invalid_record_count > 0
            || audit.identifier_coverage.missing_record_count > 0
        {
            "partial".into()
        } else {
            "structured".into()
        };
    }
    let mut digest_input = serde_json::to_value(&audit)
        .map_err(|error| DomainEvidenceProviderNormalizationError::Canonical(error.to_string()))?;
    digest_input
        .as_object_mut()
        .expect("shape audit serializes as an object")
        .remove("shape_digest");
    audit.shape_digest = canonical_digest(&digest_input)?;
    Ok(audit)
}

fn audit_fhir_payload(
    payload: &Value,
) -> Result<DomainEvidenceProviderShapeAudit, DomainEvidenceProviderNormalizationError> {
    let mut audit = shape_audit_base("fhir", payload, &["id", "identifier"], None);
    let Some(object) = payload.as_object() else {
        add_warning(
            &mut audit,
            "FHIR payload must be an object with a resourceType field",
        );
        return finish_shape_audit(audit);
    };
    if !object
        .get("resourceType")
        .is_some_and(|value| value.as_str().is_some_and(|text| !text.trim().is_empty()))
    {
        add_missing_field(&mut audit, "resourceType");
        add_warning(
            &mut audit,
            "FHIR payload lacks a non-empty resourceType field",
        );
        return finish_shape_audit(audit);
    }
    if object.get("resourceType").and_then(Value::as_str) == Some("Bundle") {
        audit.recognized_container = Some("entry".into());
        let Some(entries) = object.get("entry").and_then(Value::as_array) else {
            audit.invalid_record_count = 1;
            add_missing_field(&mut audit, "entry.array");
            add_warning(
                &mut audit,
                "FHIR Bundle entry is missing or is not an array",
            );
            return finish_shape_audit(audit);
        };
        audit.record_count = entries.len();
        for entry in entries {
            let Some(entry) = entry.as_object() else {
                audit.invalid_record_count += 1;
                add_missing_field(&mut audit, "entry.object");
                continue;
            };
            let Some(resource) = entry.get("resource").and_then(Value::as_object) else {
                audit.invalid_record_count += 1;
                add_missing_field(&mut audit, "entry.resource");
                continue;
            };
            audit.valid_record_count += 1;
            if record_has_any_field(resource, &["id", "identifier"]) {
                audit.identifier_coverage.present_record_count += 1;
            } else {
                audit.identifier_coverage.missing_record_count += 1;
                add_missing_field(&mut audit, "resource.identifier");
            }
        }
        if entries.is_empty() {
            add_warning(
                &mut audit,
                "recognized FHIR Bundle entry container is empty",
            );
        }
        if audit.invalid_record_count > 0 {
            add_warning(
                &mut audit,
                "one or more FHIR Bundle entries lack an object resource",
            );
        }
        if audit.identifier_coverage.missing_record_count > 0 {
            add_warning(
                &mut audit,
                "one or more FHIR resources lack a recognized identifier field",
            );
        }
    } else {
        audit.recognized_container = Some("resource".into());
        audit.record_count = 1;
        audit.valid_record_count = 1;
        if record_has_any_field(object, &["id", "identifier"]) {
            audit.identifier_coverage.present_record_count = 1;
        } else {
            audit.identifier_coverage.missing_record_count = 1;
            add_missing_field(&mut audit, "resource.identifier");
            add_warning(
                &mut audit,
                "FHIR resource lacks a recognized identifier field",
            );
        }
    }
    finish_shape_audit(audit)
}

fn audit_provider_payload(
    connector_kind: &str,
    payload: &Value,
) -> Result<DomainEvidenceProviderShapeAudit, DomainEvidenceProviderNormalizationError> {
    let (identifier_fields, containers, digest_fields): (&[&str], &[&str], Option<&[&str]>) =
        match connector_kind {
            "literature" => (
                &["id", "pmid", "doi", "source_id"],
                &["records", "results"],
                None,
            ),
            "clinical_trial" => (
                &["id", "nct_id", "trial_id", "source_id"],
                &["studies", "trials"],
                None,
            ),
            "object_store" => (
                &["key", "path", "uri"],
                &["objects", "files"],
                Some(&["content_digest"]),
            ),
            "provider_api" => (
                &["id", "key", "source_id", "external_id", "identifier"],
                &["records", "results", "items", "data"],
                None,
            ),
            "fhir" => return audit_fhir_payload(payload),
            _ => unreachable!("connector kind was validated before shape auditing"),
        };
    let mut audit = shape_audit_base(connector_kind, payload, identifier_fields, digest_fields);
    if connector_kind == "provider_api" && payload.is_array() {
        audit_array_container(
            &mut audit,
            "$root",
            Some(payload),
            identifier_fields,
            digest_fields,
        );
        return finish_shape_audit(audit);
    }
    let Some(object) = payload.as_object() else {
        add_warning(
            &mut audit,
            "connector payload must be an object for its named container audit",
        );
        return finish_shape_audit(audit);
    };
    let Some(container) = containers
        .iter()
        .copied()
        .find(|container| object.contains_key(*container))
    else {
        add_warning(
            &mut audit,
            "no connector-specific record container was recognized",
        );
        return finish_shape_audit(audit);
    };
    audit_array_container(
        &mut audit,
        container,
        object.get(container),
        identifier_fields,
        digest_fields,
    );
    finish_shape_audit(audit)
}

fn audit_provider_shape(
    connector_kind: &str,
    payload: &Value,
) -> Result<DomainEvidenceProviderShapeAudit, DomainEvidenceProviderNormalizationError> {
    audit_provider_payload(connector_kind, payload)
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
    let shape_audit = audit_provider_shape(&connector_kind, &request.payload)?;
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
        shape_audit,
        intake_arguments,
        guarantees: vec![
            "provider metadata and payload receive separate explicit structural identities".into(),
            "caller-supplied status remains visible and is never inferred from payload shape".into(),
            "connector-specific shape status and field-presence counts are deterministic and value-free".into(),
            "the normalized envelope can be passed to the existing digest-bound intake boundary".into(),
        ],
        limitations: vec![
            "the route does not contact or authenticate the named provider".into(),
            "payload fields are retained structurally and are not interpreted as scientific or clinical facts".into(),
            "external signatures, terminology expansion, retrieval completeness, and provenance remain caller-managed".into(),
        ],
    })
}

fn finish_replay_verification(
    mut verification: DomainEvidenceProviderReplayVerification,
) -> Result<DomainEvidenceProviderReplayVerification, DomainEvidenceProviderNormalizationError> {
    let mut digest_input = serde_json::to_value(&verification)
        .map_err(|error| DomainEvidenceProviderNormalizationError::Canonical(error.to_string()))?;
    digest_input
        .as_object_mut()
        .expect("replay verification serializes as an object")
        .remove("replay_digest");
    verification.replay_digest = canonical_digest(&digest_input)?;
    ensure_size(&serde_json::to_value(&verification).map_err(|error| {
        DomainEvidenceProviderNormalizationError::Canonical(error.to_string())
    })?)?;
    Ok(verification)
}

/// Verify a caller-managed provider observation against retained content identities.
///
/// This is a comparison operation only. It re-normalizes the supplied payload and recomputes
/// the ordinary intake digest in memory; it never fetches a provider, reads a filesystem path,
/// or treats a match as proof of authenticity or domain validity.
pub fn verify_domain_evidence_provider_replay(
    request: &DomainEvidenceProviderReplayRequest,
) -> Result<DomainEvidenceProviderReplayVerification, DomainEvidenceProviderNormalizationError> {
    let expected_payload_digest =
        digest("expected_payload_digest", &request.expected_payload_digest)?;
    let expected_request_digest = request
        .expected_request_digest
        .as_deref()
        .map(|value| digest("expected_request_digest", value))
        .transpose()?;
    let expected_shape_digest = digest("expected_shape_digest", &request.expected_shape_digest)?;
    let expected_normalization_digest = digest(
        "expected_normalization_digest",
        &request.expected_normalization_digest,
    )?;
    let expected_intake_digest = digest("expected_intake_digest", &request.expected_intake_digest)?;
    let normalized = normalize_domain_evidence_provider(&request.observation)?;
    let normalization_value = serde_json::to_value(&normalized)
        .map_err(|error| DomainEvidenceProviderNormalizationError::Canonical(error.to_string()))?;
    let observed_normalization_digest = canonical_digest(&normalization_value)?;
    let intake =
        crate::domain_evidence_intake::intake_domain_evidence(&normalized.intake_arguments)
            .map_err(|error| {
                DomainEvidenceProviderNormalizationError::Canonical(error.to_string())
            })?;
    let observed_intake_digest = intake
        .get("intake_digest")
        .and_then(Value::as_str)
        .ok_or_else(|| {
            DomainEvidenceProviderNormalizationError::Canonical(
                "recomputed intake omitted intake_digest".into(),
            )
        })?
        .to_string();
    let mut matches: BTreeMap<String, bool> = BTreeMap::new();
    matches.insert(
        "payload_digest".into(),
        expected_payload_digest == normalized.payload_digest,
    );
    matches.insert(
        "request_digest".into(),
        expected_request_digest == normalized.request_digest,
    );
    matches.insert(
        "shape_digest".into(),
        expected_shape_digest == normalized.shape_audit.shape_digest,
    );
    matches.insert(
        "normalization_digest".into(),
        expected_normalization_digest == observed_normalization_digest,
    );
    matches.insert(
        "intake_digest".into(),
        expected_intake_digest == observed_intake_digest,
    );
    let differences = matches
        .iter()
        .filter_map(|(name, matched)| (!matched).then_some(name.clone()))
        .collect::<Vec<_>>();
    let matched = differences.is_empty();
    finish_replay_verification(DomainEvidenceProviderReplayVerification {
        schema: DOMAIN_EVIDENCE_PROVIDER_REPLAY_SCHEMA.into(),
        workflow: DOMAIN_EVIDENCE_PROVIDER_REPLAY_WORKFLOW.into(),
        replay_status: if matched {
            "matched".into()
        } else {
            "mismatch".into()
        },
        matched,
        group_id: normalized.group_id,
        domains: normalized.domains,
        subject_id: normalized.subject_id,
        source_tool: normalized.source_tool,
        connector_kind: normalized.connector_kind,
        provider: normalized.provider,
        expected_payload_digest,
        observed_payload_digest: normalized.payload_digest,
        expected_request_digest,
        observed_request_digest: normalized.request_digest,
        expected_shape_digest,
        observed_shape_digest: normalized.shape_audit.shape_digest.clone(),
        expected_normalization_digest,
        observed_normalization_digest,
        expected_intake_digest,
        observed_intake_digest,
        matches,
        differences,
        shape_audit: normalized.shape_audit,
        replay_digest: String::new(),
        guarantees: vec![
            "replay compares independently recomputed payload, request, shape, normalization, and intake identities".into(),
            "a mismatch remains a structured comparison result and never becomes an observed provider success".into(),
            "the replay record is content-addressable and can be indexed idempotently by the artifact registry".into(),
        ],
        limitations: vec![
            "the operation does not contact or authenticate the provider and does not re-execute a request".into(),
            "a digest match proves identity of the supplied JSON under the canonicalization contract, not scientific, clinical, causal, regulatory, or provenance validity".into(),
            "the caller must retain and supply the expected digests; omitted expectations are not inferred".into(),
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
        assert_eq!(normalized.shape_audit.status, "structured");
        assert_eq!(normalized.shape_audit.record_count, 1);
        assert_eq!(
            normalized
                .shape_audit
                .identifier_coverage
                .present_record_count,
            1
        );
        assert_eq!(
            normalized
                .shape_audit
                .identifier_coverage
                .missing_record_count,
            0
        );
        assert!(!serde_json::to_string(&normalized.shape_audit)
            .unwrap()
            .contains("pmid:1"));
        assert_eq!(
            normalized.intake_arguments["source_plan_digest"],
            json!("b".repeat(64))
        );
        assert_eq!(normalized.payload_digest.len(), 64);
    }

    #[test]
    fn audits_all_connector_families_without_echoing_payload_values() {
        let cases = [
            (
                "literature",
                json!({"results": [{"doi": "10.1000/opaque"}]}),
                "structured",
                "results",
            ),
            (
                "clinical_trial",
                json!({"studies": [{"nct_id": "NCT-opaque"}, "malformed"]}),
                "partial",
                "studies",
            ),
            (
                "fhir",
                json!({"resourceType": "Bundle", "entry": [{"resource": {"resourceType": "Patient", "id": "opaque"}}]}),
                "structured",
                "entry",
            ),
            (
                "object_store",
                json!({"objects": [{"key": "opaque/path", "content_digest": "opaque-digest"}]}),
                "structured",
                "objects",
            ),
            (
                "provider_api",
                json!([{"external_id": "opaque"}]),
                "structured",
                "$root",
            ),
        ];
        for (connector_kind, payload, expected_status, expected_container) in cases {
            let mut request = base_request();
            request.connector_kind = connector_kind.into();
            request.payload = payload.clone();
            let normalized = normalize_domain_evidence_provider(&request).unwrap();
            assert_eq!(
                normalized.shape_audit.status, expected_status,
                "{connector_kind}"
            );
            assert_eq!(
                normalized.shape_audit.recognized_container.as_deref(),
                Some(expected_container),
                "{connector_kind}"
            );
            let serialized = serde_json::to_string(&normalized.shape_audit).unwrap();
            assert!(!serialized.contains("opaque"), "{connector_kind}");
            assert_eq!(normalized.shape_audit.shape_digest.len(), 64);
        }
    }

    #[test]
    fn distinguishes_unclassified_and_refused_shapes() {
        let mut request = base_request();
        request.payload = json!({"unexpected": {"value": "opaque"}});
        let unclassified = normalize_domain_evidence_provider(&request).unwrap();
        assert_eq!(unclassified.shape_audit.status, "unclassified");
        assert!(unclassified.shape_audit.recognized_container.is_none());
        assert!(unclassified
            .shape_audit
            .warnings
            .iter()
            .any(|warning| { warning.contains("no connector-specific record container") }));

        request.payload = json!({"records": {"id": "opaque"}});
        let refused = normalize_domain_evidence_provider(&request).unwrap();
        assert_eq!(refused.shape_audit.status, "refused");
        assert!(refused
            .shape_audit
            .missing_fields
            .iter()
            .any(|field| field == "records.array"));
    }

    #[test]
    fn shape_digest_is_deterministic_and_payload_independent() {
        let mut first = base_request();
        first.payload = json!({"records": [{"id": "first", "title": "one"}]});
        let mut second = first.clone();
        second.payload = json!({"records": [{"id": "second", "title": "two"}]});
        let first_audit = normalize_domain_evidence_provider(&first)
            .unwrap()
            .shape_audit;
        let second_audit = normalize_domain_evidence_provider(&second)
            .unwrap()
            .shape_audit;
        assert_eq!(first_audit.shape_digest, second_audit.shape_digest);
        assert_eq!(first_audit, second_audit);
    }

    fn replay_request_for(
        observation: DomainEvidenceProviderNormalizationRequest,
    ) -> DomainEvidenceProviderReplayRequest {
        let normalized = normalize_domain_evidence_provider(&observation).unwrap();
        let normalization_value = serde_json::to_value(&normalized).unwrap();
        let intake =
            crate::domain_evidence_intake::intake_domain_evidence(&normalized.intake_arguments)
                .unwrap();
        DomainEvidenceProviderReplayRequest {
            expected_payload_digest: normalized.payload_digest.clone(),
            expected_request_digest: normalized.request_digest.clone(),
            expected_shape_digest: normalized.shape_audit.shape_digest.clone(),
            expected_normalization_digest: canonical_digest(&normalization_value).unwrap(),
            expected_intake_digest: intake["intake_digest"].as_str().unwrap().into(),
            observation,
        }
    }

    #[test]
    fn replay_verification_matches_and_is_value_free() {
        let replay =
            verify_domain_evidence_provider_replay(&replay_request_for(base_request())).unwrap();
        assert!(replay.matched);
        assert_eq!(replay.replay_status, "matched");
        assert!(replay.differences.is_empty());
        assert_eq!(replay.matches.len(), 5);
        assert_eq!(replay.replay_digest.len(), 64);
        assert!(!serde_json::to_string(&replay).unwrap().contains("opaque"));
    }

    #[test]
    fn replay_verification_reports_payload_and_downstream_digest_drift() {
        let mut expected = replay_request_for(base_request());
        expected.observation.payload =
            json!({"records": [{"id": "pmid:changed", "title": "changed"}]});
        let replay = verify_domain_evidence_provider_replay(&expected).unwrap();
        assert!(!replay.matched);
        assert_eq!(replay.replay_status, "mismatch");
        assert!(replay
            .differences
            .iter()
            .any(|difference| difference == "payload_digest"));
        assert!(replay
            .differences
            .iter()
            .any(|difference| difference == "normalization_digest"));
        assert!(replay
            .differences
            .iter()
            .any(|difference| difference == "intake_digest"));
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
