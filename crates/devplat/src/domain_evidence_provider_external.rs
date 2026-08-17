//! Digest-bound receipts for caller-managed, out-of-line provider payloads.
//!
//! Provider responses can be much larger than a JSON-RPC frame or an artifact-registry record.
//! This module records the exact identity and storage handoff metadata without copying bytes into
//! the core. The caller owns the store, transfer, credentials, and later normalization; the
//! receipt makes those boundaries durable and replayable rather than implicit.

use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const DOMAIN_EVIDENCE_PROVIDER_EXTERNAL_PAYLOAD_SCHEMA: &str =
    "bioprism-devplat-domain-evidence-provider-external-payload-receipt/0.1";
pub const DOMAIN_EVIDENCE_PROVIDER_EXTERNAL_PAYLOAD_WORKFLOW: &str =
    "domain_evidence_provider_external_payload_receipt";
pub const DOMAIN_EVIDENCE_PROVIDER_EXTERNAL_PAYLOAD_REPLAY_SCHEMA: &str =
    "bioprism-devplat-domain-evidence-provider-external-payload-replay/0.1";
pub const DOMAIN_EVIDENCE_PROVIDER_EXTERNAL_PAYLOAD_REPLAY_WORKFLOW: &str =
    "domain_evidence_provider_external_payload_replay_verify";
pub const MAX_DOMAIN_EVIDENCE_PROVIDER_EXTERNAL_PAYLOAD_BYTES: usize = 256 * 1024;
pub const MAX_DOMAIN_EVIDENCE_PROVIDER_EXTERNAL_PAYLOAD_TEXT_BYTES: usize = 512;
pub const MAX_DOMAIN_EVIDENCE_PROVIDER_EXTERNAL_PAYLOAD_DOMAINS: usize = 64;
pub const MAX_DOMAIN_EVIDENCE_PROVIDER_EXTERNAL_PAYLOAD_PARENTS: usize = 128;

const CONNECTOR_KINDS: &[&str] = &[
    "literature",
    "clinical_trial",
    "fhir",
    "object_store",
    "provider_api",
];
const STORAGE_BACKENDS: &[&str] = &["object_store", "file", "database", "caller_managed"];
const LOCATOR_KINDS: &[&str] = &["opaque", "uri", "path"];
const AVAILABILITY: &[&str] = &["available", "partial", "missing", "unknown"];
const RETENTION: &[&str] = &["ephemeral", "durable", "unknown"];

fn default_availability() -> String {
    "unknown".into()
}

fn default_retention() -> String {
    "unknown".into()
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct DomainEvidenceProviderExternalPayloadReceiptRequest {
    pub group_id: String,
    pub domains: Vec<String>,
    pub subject_id: String,
    pub source_tool: String,
    pub provider: String,
    pub connector_kind: String,
    pub handoff_digest: String,
    pub transfer_id: String,
    pub payload_digest: String,
    pub byte_length: u64,
    pub storage_backend: String,
    pub locator_kind: String,
    pub locator: String,
    #[serde(default)]
    pub content_type: Option<String>,
    #[serde(default)]
    pub content_encoding: Option<String>,
    #[serde(default)]
    pub request_digest: Option<String>,
    #[serde(default)]
    pub parent_digests: Vec<String>,
    #[serde(default = "default_availability")]
    pub availability: String,
    #[serde(default = "default_retention")]
    pub retention: String,
    #[serde(default)]
    pub attempt_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DomainEvidenceProviderExternalPayloadReceipt {
    pub schema: String,
    pub workflow: String,
    pub group_id: String,
    pub domains: Vec<String>,
    pub subject_id: String,
    pub source_tool: String,
    pub provider: String,
    pub connector_kind: String,
    pub handoff_digest: String,
    pub transfer_id: String,
    pub payload_digest: String,
    pub byte_length: u64,
    pub storage_backend: String,
    pub locator_kind: String,
    pub locator: String,
    pub content_type: Option<String>,
    pub content_encoding: Option<String>,
    pub request_digest: Option<String>,
    pub parent_digests: Vec<String>,
    pub availability: String,
    pub retention: String,
    pub attempt_id: Option<String>,
    pub receipt_digest: String,
    pub execution: String,
    pub readiness_claimed: bool,
    pub guarantees: Vec<String>,
    pub limitations: Vec<String>,
}

/// Re-check the identity metadata of a caller-owned external payload without contacting its
/// storage backend. The flattened receipt request contains no payload field by construction.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct DomainEvidenceProviderExternalPayloadReplayRequest {
    #[serde(flatten)]
    pub receipt: DomainEvidenceProviderExternalPayloadReceiptRequest,
    pub expected_receipt_digest: String,
    pub expected_handoff_digest: String,
    pub expected_payload_digest: String,
    pub expected_byte_length: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DomainEvidenceProviderExternalPayloadReplayVerification {
    pub schema: String,
    pub workflow: String,
    pub replay_status: String,
    pub matched: bool,
    pub group_id: String,
    pub domains: Vec<String>,
    pub subject_id: String,
    pub source_tool: String,
    pub provider: String,
    pub connector_kind: String,
    pub expected_receipt_digest: String,
    pub observed_receipt_digest: String,
    pub expected_handoff_digest: String,
    pub observed_handoff_digest: String,
    pub expected_payload_digest: String,
    pub observed_payload_digest: String,
    pub expected_byte_length: u64,
    pub observed_byte_length: u64,
    pub matches: std::collections::BTreeMap<String, bool>,
    pub differences: Vec<String>,
    pub receipt: DomainEvidenceProviderExternalPayloadReceipt,
    pub replay_digest: String,
    pub guarantees: Vec<String>,
    pub limitations: Vec<String>,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum DomainEvidenceProviderExternalPayloadError {
    #[error("{field} must be non-empty text no longer than {MAX_DOMAIN_EVIDENCE_PROVIDER_EXTERNAL_PAYLOAD_TEXT_BYTES} bytes")]
    InvalidText { field: &'static str },
    #[error("{field} must be one of: {allowed}")]
    InvalidChoice {
        field: &'static str,
        allowed: String,
    },
    #[error("connector_kind is unsupported: {0}")]
    UnsupportedConnector(String),
    #[error("domains must contain between 1 and {MAX_DOMAIN_EVIDENCE_PROVIDER_EXTERNAL_PAYLOAD_DOMAINS} values")]
    InvalidDomains,
    #[error("{field} is not a valid lowercase SHA-256 digest: {value}")]
    InvalidDigest { field: &'static str, value: String },
    #[error("byte_length must be between 1 and {MAX_EXTERNAL_PAYLOAD_BYTES} bytes")]
    InvalidByteLength { byte_length: u64 },
    #[error("locator must not contain embedded credentials or control line breaks")]
    InvalidLocator,
    #[error("parent digests exceed the {MAX_DOMAIN_EVIDENCE_PROVIDER_EXTERNAL_PAYLOAD_PARENTS}-item bound")]
    TooManyParents,
    #[error("handoff_digest must identify the connector handoff")]
    InvalidHandoff,
    #[error("cannot canonicalize external payload receipt: {0}")]
    Canonical(String),
    #[error("external payload receipt exceeds the {MAX_DOMAIN_EVIDENCE_PROVIDER_EXTERNAL_PAYLOAD_BYTES}-byte safety bound")]
    TooLarge,
}

const MAX_EXTERNAL_PAYLOAD_BYTES: u64 = 64 * 1024 * 1024 * 1024;

fn text(
    field: &'static str,
    value: &str,
) -> Result<String, DomainEvidenceProviderExternalPayloadError> {
    if value.trim().is_empty()
        || value.len() > MAX_DOMAIN_EVIDENCE_PROVIDER_EXTERNAL_PAYLOAD_TEXT_BYTES
        || value.chars().any(char::is_control)
    {
        return Err(DomainEvidenceProviderExternalPayloadError::InvalidText { field });
    }
    Ok(value.to_owned())
}

fn digest(
    field: &'static str,
    value: &str,
) -> Result<String, DomainEvidenceProviderExternalPayloadError> {
    let value = text(field, value)?;
    if value.len() != 64
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    {
        return Err(DomainEvidenceProviderExternalPayloadError::InvalidDigest { field, value });
    }
    ContentHash::parse(value.clone()).map_err(|_| {
        DomainEvidenceProviderExternalPayloadError::InvalidDigest {
            field,
            value: value.clone(),
        }
    })?;
    Ok(value)
}

fn choice(
    field: &'static str,
    value: &str,
    allowed: &[&str],
) -> Result<String, DomainEvidenceProviderExternalPayloadError> {
    let value = text(field, value)?;
    if !allowed.contains(&value.as_str()) {
        return Err(DomainEvidenceProviderExternalPayloadError::InvalidChoice {
            field,
            allowed: allowed.join(", "),
        });
    }
    Ok(value)
}

fn domains(values: &[String]) -> Result<Vec<String>, DomainEvidenceProviderExternalPayloadError> {
    if values.is_empty() || values.len() > MAX_DOMAIN_EVIDENCE_PROVIDER_EXTERNAL_PAYLOAD_DOMAINS {
        return Err(DomainEvidenceProviderExternalPayloadError::InvalidDomains);
    }
    let mut result = BTreeSet::new();
    for value in values {
        result.insert(text("domain", value)?);
    }
    if result.len() != values.len() {
        return Err(DomainEvidenceProviderExternalPayloadError::InvalidDomains);
    }
    Ok(result.into_iter().collect())
}

fn optional_text(
    field: &'static str,
    value: &Option<String>,
) -> Result<Option<String>, DomainEvidenceProviderExternalPayloadError> {
    value.as_deref().map(|value| text(field, value)).transpose()
}

fn parent_digests(
    values: &[String],
) -> Result<Vec<String>, DomainEvidenceProviderExternalPayloadError> {
    if values.len() > MAX_DOMAIN_EVIDENCE_PROVIDER_EXTERNAL_PAYLOAD_PARENTS {
        return Err(DomainEvidenceProviderExternalPayloadError::TooManyParents);
    }
    let mut result = values
        .iter()
        .map(|value| digest("parent_digest", value))
        .collect::<Result<Vec<_>, _>>()?;
    result.sort();
    result.dedup();
    Ok(result)
}

fn canonical_digest(value: &Value) -> Result<String, DomainEvidenceProviderExternalPayloadError> {
    ContentHash::of_value(value)
        .map(|digest| digest.to_string())
        .map_err(|error| DomainEvidenceProviderExternalPayloadError::Canonical(error.to_string()))
}

/// Validate and record an out-of-line payload transfer. No store or provider is contacted.
pub fn record_domain_evidence_provider_external_payload(
    request: &DomainEvidenceProviderExternalPayloadReceiptRequest,
) -> Result<DomainEvidenceProviderExternalPayloadReceipt, DomainEvidenceProviderExternalPayloadError>
{
    let group_id = text("group_id", &request.group_id)?;
    let domains = domains(&request.domains)?;
    let subject_id = text("subject_id", &request.subject_id)?;
    let source_tool = text("source_tool", &request.source_tool)?;
    let provider = text("provider", &request.provider)?;
    let connector_kind = text("connector_kind", &request.connector_kind)?;
    if !CONNECTOR_KINDS.contains(&connector_kind.as_str()) {
        return Err(
            DomainEvidenceProviderExternalPayloadError::UnsupportedConnector(connector_kind),
        );
    }
    let handoff_digest = digest("handoff_digest", &request.handoff_digest)?;
    let transfer_id = text("transfer_id", &request.transfer_id)?;
    let payload_digest = digest("payload_digest", &request.payload_digest)?;
    if !(1..=MAX_EXTERNAL_PAYLOAD_BYTES).contains(&request.byte_length) {
        return Err(
            DomainEvidenceProviderExternalPayloadError::InvalidByteLength {
                byte_length: request.byte_length,
            },
        );
    }
    let storage_backend = choice(
        "storage_backend",
        &request.storage_backend,
        STORAGE_BACKENDS,
    )?;
    let locator_kind = choice("locator_kind", &request.locator_kind, LOCATOR_KINDS)?;
    let locator = text("locator", &request.locator)?;
    if locator.contains(['\r', '\n']) {
        return Err(DomainEvidenceProviderExternalPayloadError::InvalidLocator);
    }
    if let Some(scheme_end) = locator.find("://") {
        let authority = &locator[scheme_end + 3..];
        let authority_end = authority.find(['/', '?', '#']).unwrap_or(authority.len());
        if authority[..authority_end].contains('@') {
            return Err(DomainEvidenceProviderExternalPayloadError::InvalidLocator);
        }
    }
    let content_type = optional_text("content_type", &request.content_type)?;
    let content_encoding = optional_text("content_encoding", &request.content_encoding)?;
    let request_digest = request
        .request_digest
        .as_deref()
        .map(|value| digest("request_digest", value))
        .transpose()?;
    let parent_digests = parent_digests(&request.parent_digests)?;
    let availability = choice("availability", &request.availability, AVAILABILITY)?;
    let retention = choice("retention", &request.retention, RETENTION)?;
    let attempt_id = optional_text("attempt_id", &request.attempt_id)?;
    let mut unsigned = json!({
        "schema": DOMAIN_EVIDENCE_PROVIDER_EXTERNAL_PAYLOAD_SCHEMA,
        "workflow": DOMAIN_EVIDENCE_PROVIDER_EXTERNAL_PAYLOAD_WORKFLOW,
        "group_id": group_id,
        "domains": domains,
        "subject_id": subject_id,
        "source_tool": source_tool,
        "provider": provider,
        "connector_kind": connector_kind,
        "handoff_digest": handoff_digest,
        "transfer_id": transfer_id,
        "payload_digest": payload_digest,
        "byte_length": request.byte_length,
        "storage_backend": storage_backend,
        "locator_kind": locator_kind,
        "locator": locator,
        "content_type": content_type,
        "content_encoding": content_encoding,
        "request_digest": request_digest,
        "parent_digests": parent_digests,
        "availability": availability,
        "retention": retention,
        "attempt_id": attempt_id,
        "execution": "not_started",
        "readiness_claimed": false,
        "guarantees": [
            "payload bytes remain caller-owned and are represented by an exact content digest",
            "storage, retention, availability, and transfer identity remain explicit metadata",
            "the receipt can parent later provider normalization without copying the payload through MCP"
        ],
        "limitations": [
            "the core does not fetch, authenticate, decrypt, or independently inspect the external payload",
            "locator and availability are caller assertions and do not prove store durability or accessibility",
            "payload identity does not establish provider authenticity, scientific, clinical, provenance, or release validity"
        ]
    });
    let receipt_digest = canonical_digest(&unsigned)?;
    unsigned["receipt_digest"] = json!(receipt_digest);
    let receipt: DomainEvidenceProviderExternalPayloadReceipt = serde_json::from_value(unsigned)
        .map_err(|error| {
            DomainEvidenceProviderExternalPayloadError::Canonical(error.to_string())
        })?;
    let bytes = serde_json::to_vec(&receipt).map_err(|error| {
        DomainEvidenceProviderExternalPayloadError::Canonical(error.to_string())
    })?;
    if bytes.len() > MAX_DOMAIN_EVIDENCE_PROVIDER_EXTERNAL_PAYLOAD_BYTES {
        return Err(DomainEvidenceProviderExternalPayloadError::TooLarge);
    }
    Ok(receipt)
}

/// Replay the receipt identity contract locally. This is intentionally metadata-only: it does
/// not fetch, decrypt, checksum, or otherwise inspect the bytes behind the caller locator.
pub fn verify_domain_evidence_provider_external_payload_replay(
    request: &DomainEvidenceProviderExternalPayloadReplayRequest,
) -> Result<
    DomainEvidenceProviderExternalPayloadReplayVerification,
    DomainEvidenceProviderExternalPayloadError,
> {
    let receipt = record_domain_evidence_provider_external_payload(&request.receipt)?;
    let expected_receipt_digest =
        digest("expected_receipt_digest", &request.expected_receipt_digest)?;
    let expected_handoff_digest =
        digest("expected_handoff_digest", &request.expected_handoff_digest)?;
    let expected_payload_digest =
        digest("expected_payload_digest", &request.expected_payload_digest)?;
    if !(1..=MAX_EXTERNAL_PAYLOAD_BYTES).contains(&request.expected_byte_length) {
        return Err(
            DomainEvidenceProviderExternalPayloadError::InvalidByteLength {
                byte_length: request.expected_byte_length,
            },
        );
    }
    let mut matches: BTreeMap<String, bool> = BTreeMap::new();
    matches.insert(
        "receipt_digest".into(),
        receipt.receipt_digest == expected_receipt_digest,
    );
    matches.insert(
        "handoff_digest".into(),
        receipt.handoff_digest == expected_handoff_digest,
    );
    matches.insert(
        "payload_digest".into(),
        receipt.payload_digest == expected_payload_digest,
    );
    matches.insert(
        "byte_length".into(),
        receipt.byte_length == request.expected_byte_length,
    );
    let differences = matches
        .iter()
        .filter(|(_, matched)| !*matched)
        .map(|(name, _)| name.clone())
        .collect::<Vec<_>>();
    let matched = differences.is_empty();
    let replay_status = if matched { "matched" } else { "mismatch" };
    let mut unsigned = json!({
        "schema": DOMAIN_EVIDENCE_PROVIDER_EXTERNAL_PAYLOAD_REPLAY_SCHEMA,
        "workflow": DOMAIN_EVIDENCE_PROVIDER_EXTERNAL_PAYLOAD_REPLAY_WORKFLOW,
        "replay_status": replay_status,
        "matched": matched,
        "group_id": receipt.group_id,
        "domains": receipt.domains,
        "subject_id": receipt.subject_id,
        "source_tool": receipt.source_tool,
        "provider": receipt.provider,
        "connector_kind": receipt.connector_kind,
        "expected_receipt_digest": expected_receipt_digest,
        "observed_receipt_digest": receipt.receipt_digest,
        "expected_handoff_digest": expected_handoff_digest,
        "observed_handoff_digest": receipt.handoff_digest,
        "expected_payload_digest": expected_payload_digest,
        "observed_payload_digest": receipt.payload_digest,
        "expected_byte_length": request.expected_byte_length,
        "observed_byte_length": receipt.byte_length,
        "matches": matches,
        "differences": differences,
        "receipt": receipt,
        "guarantees": [
            "receipt identity, handoff identity, payload identity, and byte length are compared deterministically",
            "replay performs no external-store or provider operation",
            "the replay artifact can be retained independently of the payload bytes"
        ],
        "limitations": [
            "a matching receipt does not prove that the locator still resolves to the bytes",
            "the core does not independently recompute the payload digest or verify storage retention",
            "payload and provider authenticity, scientific, clinical, provenance, and release validity remain unclaimed"
        ]
    });
    let replay_digest = canonical_digest(&unsigned)?;
    unsigned["replay_digest"] = json!(replay_digest);
    serde_json::from_value(unsigned)
        .map_err(|error| DomainEvidenceProviderExternalPayloadError::Canonical(error.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request() -> DomainEvidenceProviderExternalPayloadReceiptRequest {
        DomainEvidenceProviderExternalPayloadReceiptRequest {
            group_id: "biological_domains".into(),
            domains: vec!["genomics".into(), "oncology".into()],
            subject_id: "subject-1".into(),
            source_tool: "literature_bind_check".into(),
            provider: "pubmed".into(),
            connector_kind: "literature".into(),
            handoff_digest: "a".repeat(64),
            transfer_id: "transfer-1".into(),
            payload_digest: "b".repeat(64),
            byte_length: 4096,
            storage_backend: "object_store".into(),
            locator_kind: "opaque".into(),
            locator: "store://caller/pubmed/objects/1".into(),
            content_type: Some("application/json".into()),
            content_encoding: Some("gzip".into()),
            request_digest: Some("c".repeat(64)),
            parent_digests: vec!["d".repeat(64)],
            availability: "available".into(),
            retention: "durable".into(),
            attempt_id: Some("attempt-1".into()),
        }
    }

    #[test]
    fn receipt_is_value_free_digest_bound_and_canonical() {
        let receipt = record_domain_evidence_provider_external_payload(&request()).unwrap();
        assert_eq!(receipt.receipt_digest.len(), 64);
        assert_eq!(receipt.byte_length, 4096);
        assert!(!receipt.readiness_claimed);
        assert_eq!(receipt.execution, "not_started");
        let wire = serde_json::to_string(&receipt).unwrap();
        assert!(!wire.contains("credential_material"));
        assert!(!wire.contains("records"));
    }

    #[test]
    fn receipt_refuses_unsafe_locator_unknown_fields_and_unbounded_bytes() {
        let mut invalid = request();
        invalid.locator = "https://user:pass@example.org/object".into();
        assert_eq!(
            record_domain_evidence_provider_external_payload(&invalid).unwrap_err(),
            DomainEvidenceProviderExternalPayloadError::InvalidLocator
        );
        let mut invalid = request();
        invalid.byte_length = MAX_EXTERNAL_PAYLOAD_BYTES + 1;
        assert!(matches!(
            record_domain_evidence_provider_external_payload(&invalid),
            Err(DomainEvidenceProviderExternalPayloadError::InvalidByteLength { .. })
        ));
        let unknown =
            serde_json::from_value::<DomainEvidenceProviderExternalPayloadReceiptRequest>(json!({
                "group_id": "biological_domains",
                "domains": ["oncology"],
                "subject_id": "subject-1",
                "source_tool": "literature_bind_check",
                "provider": "pubmed",
                "connector_kind": "literature",
                "handoff_digest": "a".repeat(64),
                "transfer_id": "transfer-1",
                "payload_digest": "b".repeat(64),
                "byte_length": 1,
                "storage_backend": "object_store",
                "locator_kind": "opaque",
                "locator": "store://object/1",
                "credential_material": "never"
            }));
        assert!(unknown.is_err());
    }

    #[test]
    fn receipt_changes_when_external_identity_changes() {
        let first = record_domain_evidence_provider_external_payload(&request()).unwrap();
        let mut changed = request();
        changed.transfer_id = "transfer-2".into();
        let second = record_domain_evidence_provider_external_payload(&changed).unwrap();
        assert_ne!(first.receipt_digest, second.receipt_digest);
    }

    #[test]
    fn replay_matches_metadata_and_reports_digest_or_size_drift() {
        let receipt = record_domain_evidence_provider_external_payload(&request()).unwrap();
        let replay = verify_domain_evidence_provider_external_payload_replay(
            &DomainEvidenceProviderExternalPayloadReplayRequest {
                receipt: request(),
                expected_receipt_digest: receipt.receipt_digest.clone(),
                expected_handoff_digest: "a".repeat(64),
                expected_payload_digest: "b".repeat(64),
                expected_byte_length: 4096,
            },
        )
        .unwrap();
        assert!(replay.matched);
        assert_eq!(replay.replay_status, "matched");
        let mut changed = request();
        changed.byte_length = 8192;
        let drift = verify_domain_evidence_provider_external_payload_replay(
            &DomainEvidenceProviderExternalPayloadReplayRequest {
                receipt: changed,
                expected_receipt_digest: receipt.receipt_digest,
                expected_handoff_digest: "a".repeat(64),
                expected_payload_digest: "b".repeat(64),
                expected_byte_length: 4096,
            },
        )
        .unwrap();
        assert!(!drift.matched);
        assert_eq!(drift.differences, vec!["byte_length", "receipt_digest"]);
    }
}
