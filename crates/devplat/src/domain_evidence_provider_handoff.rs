//! Digest-bound handoff contract for caller-managed provider connector plugins.
//!
//! The core deliberately does not own provider credentials, network clients, or plugin
//! processes. A production integration can still publish one auditable handoff that declares its
//! connector, scope, auth posture, secret *references* (never material), request/payload
//! identities, and explicit caller status before passing the payload to provider normalization.

use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::BTreeSet;
use thiserror::Error;

pub const DOMAIN_EVIDENCE_PROVIDER_HANDOFF_SCHEMA: &str =
    "bioprism-devplat-domain-evidence-provider-connector-handoff/0.1";
pub const DOMAIN_EVIDENCE_PROVIDER_HANDOFF_WORKFLOW: &str =
    "domain_evidence_provider_connector_handoff";
pub const DOMAIN_EVIDENCE_PROVIDER_MANIFEST_SCHEMA: &str =
    "bioprism-devplat-domain-evidence-provider-connector-manifest/0.1";
pub const MAX_DOMAIN_EVIDENCE_PROVIDER_HANDOFF_BYTES: usize = 2_000_000;
pub const MAX_DOMAIN_EVIDENCE_PROVIDER_HANDOFF_TEXT_BYTES: usize = 512;
pub const MAX_DOMAIN_EVIDENCE_PROVIDER_HANDOFF_DOMAINS: usize = 64;
pub const MAX_DOMAIN_EVIDENCE_PROVIDER_HANDOFF_CAPABILITIES: usize = 64;
pub const MAX_DOMAIN_EVIDENCE_PROVIDER_HANDOFF_PARENTS: usize = 128;
pub const MAX_DOMAIN_EVIDENCE_PROVIDER_HANDOFF_SECRET_REFS: usize = 32;

const CONNECTOR_KINDS: &[&str] = &[
    "literature",
    "clinical_trial",
    "fhir",
    "object_store",
    "provider_api",
];
const HANDOFF_STATUSES: &[&str] = &[
    "prepared",
    "submitted",
    "observed",
    "partial",
    "refused",
    "error",
    "unknown",
];
const AUTH_STATUSES: &[&str] = &["none", "caller_asserted", "delegated", "unknown"];

fn default_handoff_status() -> String {
    "unknown".into()
}

fn default_auth_posture() -> DomainEvidenceProviderAuthPosture {
    DomainEvidenceProviderAuthPosture {
        status: "unknown".into(),
        secret_refs: Vec::new(),
        does_not_claim: vec![
            "credential material is not retained by the core".into(),
            "provider authentication or authorization is not verified".into(),
        ],
    }
}

/// Public connector declaration. `secret_refs` are opaque caller-owned labels only; a manifest
/// containing credential material is rejected by the MCP boundary's strict deserializer.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct DomainEvidenceProviderConnectorManifest {
    pub schema: String,
    pub connector_id: String,
    pub version: String,
    pub provider: String,
    pub connector_kind: String,
    pub domains: Vec<String>,
    pub capabilities: Vec<String>,
    pub transport: String,
    #[serde(default = "default_auth_posture")]
    pub auth_posture: DomainEvidenceProviderAuthPosture,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct DomainEvidenceProviderAuthPosture {
    pub status: String,
    #[serde(default)]
    pub secret_refs: Vec<String>,
    pub does_not_claim: Vec<String>,
}

/// Caller-owned connector handoff. The request has no payload field by design: payload bytes are
/// transferred separately to `domain_evidence_provider_normalize` and bound by `payload_digest`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct DomainEvidenceProviderHandoffRequest {
    pub group_id: String,
    pub domains: Vec<String>,
    pub subject_id: String,
    pub source_tool: String,
    pub provider: String,
    pub connector_kind: String,
    pub manifest: DomainEvidenceProviderConnectorManifest,
    #[serde(default = "default_handoff_status")]
    pub status: String,
    #[serde(default)]
    pub request_digest: Option<String>,
    #[serde(default)]
    pub payload_digest: Option<String>,
    #[serde(default)]
    pub source_plan_digest: Option<String>,
    #[serde(default)]
    pub parent_digests: Vec<String>,
    #[serde(default)]
    pub attempt_id: Option<String>,
}

/// Canonical handoff artifact suitable for idempotent artifact-registry registration.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DomainEvidenceProviderHandoff {
    pub schema: String,
    pub workflow: String,
    pub group_id: String,
    pub domains: Vec<String>,
    pub subject_id: String,
    pub source_tool: String,
    pub provider: String,
    pub connector_kind: String,
    pub status: String,
    pub manifest: DomainEvidenceProviderConnectorManifest,
    pub manifest_digest: String,
    pub request_digest: Option<String>,
    pub payload_digest: Option<String>,
    pub source_plan_digest: Option<String>,
    pub parent_digests: Vec<String>,
    pub attempt_id: Option<String>,
    pub handoff_digest: String,
    pub execution: String,
    pub readiness_claimed: bool,
    pub guarantees: Vec<String>,
    pub limitations: Vec<String>,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum DomainEvidenceProviderHandoffError {
    #[error("{field} must be non-empty text no longer than {MAX_DOMAIN_EVIDENCE_PROVIDER_HANDOFF_TEXT_BYTES} bytes")]
    InvalidText { field: &'static str },
    #[error("connector_kind must be one of: {0}")]
    UnsupportedConnector(String),
    #[error("manifest schema must be {DOMAIN_EVIDENCE_PROVIDER_MANIFEST_SCHEMA}")]
    InvalidManifestSchema,
    #[error("manifest transport must be caller_managed")]
    InvalidTransport,
    #[error("manifest connector scope does not cover the requested handoff scope")]
    ManifestScopeMismatch,
    #[error(
        "domains must contain between 1 and {MAX_DOMAIN_EVIDENCE_PROVIDER_HANDOFF_DOMAINS} values"
    )]
    InvalidDomains,
    #[error("capabilities must contain between 1 and {MAX_DOMAIN_EVIDENCE_PROVIDER_HANDOFF_CAPABILITIES} unique values")]
    InvalidCapabilities,
    #[error("auth posture status must be one of: none, caller_asserted, delegated, unknown")]
    InvalidAuthStatus,
    #[error("auth posture must declare at least one non-claim")]
    MissingAuthNonClaims,
    #[error("too many secret references")]
    TooManySecretRefs,
    #[error("too many parent digests")]
    TooManyParents,
    #[error(
        "status must be one of: prepared, submitted, observed, partial, refused, error, unknown"
    )]
    InvalidStatus(String),
    #[error("{field} is not a valid lowercase SHA-256 digest: {value}")]
    InvalidDigest { field: &'static str, value: String },
    #[error("cannot canonicalize provider handoff: {0}")]
    Canonical(String),
    #[error("provider handoff exceeds the {MAX_DOMAIN_EVIDENCE_PROVIDER_HANDOFF_BYTES}-byte safety bound")]
    TooLarge,
}

fn bounded_text(
    field: &'static str,
    value: &str,
) -> Result<String, DomainEvidenceProviderHandoffError> {
    if value.trim().is_empty()
        || value.len() > MAX_DOMAIN_EVIDENCE_PROVIDER_HANDOFF_TEXT_BYTES
        || value.chars().any(char::is_control)
    {
        return Err(DomainEvidenceProviderHandoffError::InvalidText { field });
    }
    Ok(value.to_owned())
}

fn digest(field: &'static str, value: &str) -> Result<String, DomainEvidenceProviderHandoffError> {
    let value = bounded_text(field, value)?;
    if value.len() != 64
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    {
        return Err(DomainEvidenceProviderHandoffError::InvalidDigest { field, value });
    }
    ContentHash::parse(value.clone()).map_err(|_| {
        DomainEvidenceProviderHandoffError::InvalidDigest {
            field,
            value: value.clone(),
        }
    })?;
    Ok(value)
}

fn canonical_digest(value: &Value) -> Result<String, DomainEvidenceProviderHandoffError> {
    ContentHash::of_value(value)
        .map(|digest| digest.to_string())
        .map_err(|error| DomainEvidenceProviderHandoffError::Canonical(error.to_string()))
}

fn ensure_size(value: &Value) -> Result<(), DomainEvidenceProviderHandoffError> {
    let bytes = serde_json::to_vec(value)
        .map_err(|error| DomainEvidenceProviderHandoffError::Canonical(error.to_string()))?;
    if bytes.len() > MAX_DOMAIN_EVIDENCE_PROVIDER_HANDOFF_BYTES {
        return Err(DomainEvidenceProviderHandoffError::TooLarge);
    }
    Ok(())
}

fn bounded_unique_texts(
    field: &'static str,
    values: &[String],
    maximum: usize,
) -> Result<Vec<String>, DomainEvidenceProviderHandoffError> {
    if values.is_empty() || values.len() > maximum {
        return Err(DomainEvidenceProviderHandoffError::InvalidDomains);
    }
    let mut unique = BTreeSet::new();
    for value in values {
        unique.insert(bounded_text(field, value)?);
    }
    if unique.len() != values.len() {
        return Err(DomainEvidenceProviderHandoffError::InvalidDomains);
    }
    Ok(unique.into_iter().collect())
}

fn validate_auth_posture(
    posture: &DomainEvidenceProviderAuthPosture,
) -> Result<DomainEvidenceProviderAuthPosture, DomainEvidenceProviderHandoffError> {
    if !AUTH_STATUSES.contains(&posture.status.as_str()) {
        return Err(DomainEvidenceProviderHandoffError::InvalidAuthStatus);
    }
    if posture.does_not_claim.is_empty()
        || posture
            .does_not_claim
            .iter()
            .any(|claim| bounded_text("does_not_claim", claim).is_err())
    {
        return Err(DomainEvidenceProviderHandoffError::MissingAuthNonClaims);
    }
    if posture.secret_refs.len() > MAX_DOMAIN_EVIDENCE_PROVIDER_HANDOFF_SECRET_REFS {
        return Err(DomainEvidenceProviderHandoffError::TooManySecretRefs);
    }
    let secret_refs = posture
        .secret_refs
        .iter()
        .map(|reference| bounded_text("secret_ref", reference))
        .collect::<Result<Vec<_>, _>>()?;
    let does_not_claim = posture
        .does_not_claim
        .iter()
        .map(|claim| bounded_text("does_not_claim", claim))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(DomainEvidenceProviderAuthPosture {
        status: posture.status.clone(),
        secret_refs,
        does_not_claim,
    })
}

fn validate_manifest(
    manifest: &DomainEvidenceProviderConnectorManifest,
) -> Result<DomainEvidenceProviderConnectorManifest, DomainEvidenceProviderHandoffError> {
    if manifest.schema != DOMAIN_EVIDENCE_PROVIDER_MANIFEST_SCHEMA {
        return Err(DomainEvidenceProviderHandoffError::InvalidManifestSchema);
    }
    let connector_id = bounded_text("connector_id", &manifest.connector_id)?;
    let version = bounded_text("version", &manifest.version)?;
    let provider = bounded_text("provider", &manifest.provider)?;
    let connector_kind = bounded_text("connector_kind", &manifest.connector_kind)?;
    if !CONNECTOR_KINDS.contains(&connector_kind.as_str()) {
        return Err(DomainEvidenceProviderHandoffError::UnsupportedConnector(
            connector_kind,
        ));
    }
    if manifest.transport != "caller_managed" {
        return Err(DomainEvidenceProviderHandoffError::InvalidTransport);
    }
    let domains = bounded_unique_texts(
        "manifest_domain",
        &manifest.domains,
        MAX_DOMAIN_EVIDENCE_PROVIDER_HANDOFF_DOMAINS,
    )?;
    if manifest.capabilities.is_empty()
        || manifest.capabilities.len() > MAX_DOMAIN_EVIDENCE_PROVIDER_HANDOFF_CAPABILITIES
    {
        return Err(DomainEvidenceProviderHandoffError::InvalidCapabilities);
    }
    let capabilities = bounded_unique_texts(
        "capability",
        &manifest.capabilities,
        MAX_DOMAIN_EVIDENCE_PROVIDER_HANDOFF_CAPABILITIES,
    )
    .map_err(|_| DomainEvidenceProviderHandoffError::InvalidCapabilities)?;
    let auth_posture = validate_auth_posture(&manifest.auth_posture)?;
    Ok(DomainEvidenceProviderConnectorManifest {
        schema: DOMAIN_EVIDENCE_PROVIDER_MANIFEST_SCHEMA.into(),
        connector_id,
        version,
        provider,
        connector_kind,
        domains,
        capabilities,
        transport: "caller_managed".into(),
        auth_posture,
    })
}

/// Validate and canonicalize one external connector handoff. No connector is invoked.
pub fn handoff_domain_evidence_provider(
    request: &DomainEvidenceProviderHandoffRequest,
) -> Result<DomainEvidenceProviderHandoff, DomainEvidenceProviderHandoffError> {
    let group_id = bounded_text("group_id", &request.group_id)?;
    let subject_id = bounded_text("subject_id", &request.subject_id)?;
    let source_tool = bounded_text("source_tool", &request.source_tool)?;
    let provider = bounded_text("provider", &request.provider)?;
    let connector_kind = bounded_text("connector_kind", &request.connector_kind)?;
    if !CONNECTOR_KINDS.contains(&connector_kind.as_str()) {
        return Err(DomainEvidenceProviderHandoffError::UnsupportedConnector(
            connector_kind,
        ));
    }
    let domains = bounded_unique_texts(
        "domain",
        &request.domains,
        MAX_DOMAIN_EVIDENCE_PROVIDER_HANDOFF_DOMAINS,
    )?;
    if !HANDOFF_STATUSES.contains(&request.status.as_str()) {
        return Err(DomainEvidenceProviderHandoffError::InvalidStatus(
            request.status.clone(),
        ));
    }
    let manifest = validate_manifest(&request.manifest)?;
    if manifest.provider != provider || manifest.connector_kind != connector_kind {
        return Err(DomainEvidenceProviderHandoffError::ManifestScopeMismatch);
    }
    if domains
        .iter()
        .any(|domain| !manifest.domains.iter().any(|declared| declared == domain))
    {
        return Err(DomainEvidenceProviderHandoffError::ManifestScopeMismatch);
    }
    let request_digest = request
        .request_digest
        .as_deref()
        .map(|value| digest("request_digest", value))
        .transpose()?;
    let payload_digest = request
        .payload_digest
        .as_deref()
        .map(|value| digest("payload_digest", value))
        .transpose()?;
    let source_plan_digest = request
        .source_plan_digest
        .as_deref()
        .map(|value| digest("source_plan_digest", value))
        .transpose()?;
    if request.parent_digests.len() > MAX_DOMAIN_EVIDENCE_PROVIDER_HANDOFF_PARENTS {
        return Err(DomainEvidenceProviderHandoffError::TooManyParents);
    }
    let mut parent_digests = request
        .parent_digests
        .iter()
        .map(|value| digest("parent_digest", value))
        .collect::<Result<Vec<_>, _>>()?;
    parent_digests.sort();
    parent_digests.dedup();
    let attempt_id = request
        .attempt_id
        .as_deref()
        .map(|value| bounded_text("attempt_id", value))
        .transpose()?;
    let manifest_value = serde_json::to_value(&manifest)
        .map_err(|error| DomainEvidenceProviderHandoffError::Canonical(error.to_string()))?;
    let manifest_digest = canonical_digest(&manifest_value)?;
    let mut unsigned = json!({
        "schema": DOMAIN_EVIDENCE_PROVIDER_HANDOFF_SCHEMA,
        "workflow": DOMAIN_EVIDENCE_PROVIDER_HANDOFF_WORKFLOW,
        "group_id": group_id,
        "domains": domains,
        "subject_id": subject_id,
        "source_tool": source_tool,
        "provider": provider,
        "connector_kind": connector_kind,
        "status": request.status,
        "manifest": manifest,
        "manifest_digest": manifest_digest,
        "request_digest": request_digest,
        "payload_digest": payload_digest,
        "source_plan_digest": source_plan_digest,
        "parent_digests": parent_digests,
        "attempt_id": attempt_id,
        "execution": "not_started",
        "readiness_claimed": false,
        "guarantees": [
            "the external connector declares scope, capability, and auth posture before payload intake",
            "secret references are retained only as caller labels and never as credential material",
            "request and payload identities can be bound to the later provider-normalization artifact"
        ],
        "limitations": [
            "the core does not launch, authenticate, or contact the connector",
            "caller status and auth posture are declarations, not independently verified observations",
            "a handoff does not establish provider authenticity, retrieval completeness, scientific, clinical, or provenance validity"
        ]
    });
    let handoff_digest = canonical_digest(&unsigned)?;
    unsigned["handoff_digest"] = json!(handoff_digest);
    let handoff: DomainEvidenceProviderHandoff = serde_json::from_value(unsigned)
        .map_err(|error| DomainEvidenceProviderHandoffError::Canonical(error.to_string()))?;
    ensure_size(
        &serde_json::to_value(&handoff)
            .map_err(|error| DomainEvidenceProviderHandoffError::Canonical(error.to_string()))?,
    )?;
    Ok(handoff)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request() -> DomainEvidenceProviderHandoffRequest {
        DomainEvidenceProviderHandoffRequest {
            group_id: "biological_domains".into(),
            domains: vec!["oncology".into()],
            subject_id: "subject-1".into(),
            source_tool: "literature_bind_check".into(),
            provider: "pubmed".into(),
            connector_kind: "literature".into(),
            manifest: DomainEvidenceProviderConnectorManifest {
                schema: DOMAIN_EVIDENCE_PROVIDER_MANIFEST_SCHEMA.into(),
                connector_id: "caller.pubmed".into(),
                version: "1.2.0".into(),
                provider: "pubmed".into(),
                connector_kind: "literature".into(),
                domains: vec!["oncology".into(), "genomics".into()],
                capabilities: vec!["query".into(), "retain".into()],
                transport: "caller_managed".into(),
                auth_posture: DomainEvidenceProviderAuthPosture {
                    status: "caller_asserted".into(),
                    secret_refs: vec!["secret://caller/pubmed".into()],
                    does_not_claim: vec!["provider authentication".into()],
                },
            },
            status: "observed".into(),
            request_digest: Some("a".repeat(64)),
            payload_digest: Some("b".repeat(64)),
            source_plan_digest: Some("c".repeat(64)),
            parent_digests: vec!["d".repeat(64)],
            attempt_id: Some("attempt-1".into()),
        }
    }

    #[test]
    fn canonicalizes_scoped_handoff_without_credential_material() {
        let handoff = handoff_domain_evidence_provider(&request()).unwrap();
        assert_eq!(handoff.status, "observed");
        assert_eq!(handoff.manifest_digest.len(), 64);
        assert_eq!(handoff.handoff_digest.len(), 64);
        assert_eq!(handoff.parent_digests, vec!["d".repeat(64)]);
        let wire = serde_json::to_string(&handoff).unwrap();
        assert!(wire.contains("secret://caller/pubmed"));
        assert!(!wire.contains("credential_material"));
    }

    #[test]
    fn rejects_manifest_scope_transport_and_unknown_fields() {
        let mut invalid = request();
        invalid.manifest.transport = "http".into();
        assert_eq!(
            handoff_domain_evidence_provider(&invalid).unwrap_err(),
            DomainEvidenceProviderHandoffError::InvalidTransport
        );
        let mut invalid = request();
        invalid.domains = vec!["imaging".into()];
        assert_eq!(
            handoff_domain_evidence_provider(&invalid).unwrap_err(),
            DomainEvidenceProviderHandoffError::ManifestScopeMismatch
        );
        let unknown = serde_json::from_value::<DomainEvidenceProviderHandoffRequest>(json!({
            "group_id": "biological_domains",
            "domains": ["oncology"],
            "subject_id": "subject-1",
            "source_tool": "literature_bind_check",
            "provider": "pubmed",
            "connector_kind": "literature",
            "manifest": {
                "schema": DOMAIN_EVIDENCE_PROVIDER_MANIFEST_SCHEMA,
                "connector_id": "caller.pubmed",
                "version": "1.0.0",
                "provider": "pubmed",
                "connector_kind": "literature",
                "domains": ["oncology"],
                "capabilities": ["query"],
                "transport": "caller_managed",
                "auth_posture": {"status": "unknown", "does_not_claim": ["auth"]}
            },
            "credential_material": "never"
        }));
        assert!(unknown.is_err());
    }

    #[test]
    fn handoff_digest_changes_when_binding_identity_changes() {
        let first = handoff_domain_evidence_provider(&request()).unwrap();
        let mut changed = request();
        changed.payload_digest = Some("e".repeat(64));
        let second = handoff_domain_evidence_provider(&changed).unwrap();
        assert_ne!(first.handoff_digest, second.handoff_digest);
    }
}
