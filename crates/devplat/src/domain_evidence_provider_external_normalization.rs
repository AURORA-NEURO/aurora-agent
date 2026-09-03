//! Digest-verified materialization of caller-owned external provider payloads.
//!
//! Large provider payloads remain out-of-line until a caller explicitly supplies a bounded JSON
//! materialization. The bridge recomputes the canonical JSON digest, requires it to equal the
//! external receipt's payload digest, then sends the verified value through the ordinary provider
//! normalization and catalogue-bound intake path. It never opens the receipt locator or contacts
//! a provider.

use crate::domain_evidence_provider::{
    normalize_domain_evidence_provider, DomainEvidenceProviderNormalization,
    DomainEvidenceProviderNormalizationError, DomainEvidenceProviderNormalizationRequest,
};
use crate::domain_evidence_provider_external::{
    record_domain_evidence_provider_external_payload, DomainEvidenceProviderExternalPayloadError,
    DomainEvidenceProviderExternalPayloadReceipt,
    DomainEvidenceProviderExternalPayloadReceiptRequest,
};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use thiserror::Error;

pub const DOMAIN_EVIDENCE_PROVIDER_EXTERNAL_PAYLOAD_NORMALIZATION_SCHEMA: &str =
    "bioprism-devplat-domain-evidence-provider-external-payload-normalization/0.1";
pub const DOMAIN_EVIDENCE_PROVIDER_EXTERNAL_PAYLOAD_NORMALIZATION_WORKFLOW: &str =
    "domain_evidence_provider_external_payload_normalize";

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

/// Caller-materialized JSON plus the external receipt that is expected to identify it.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct DomainEvidenceProviderExternalPayloadNormalizationRequest {
    #[serde(flatten)]
    pub receipt: DomainEvidenceProviderExternalPayloadReceiptRequest,
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

/// Verified external identity and the ordinary normalized envelope produced from it.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DomainEvidenceProviderExternalPayloadNormalization {
    pub schema: String,
    pub workflow: String,
    pub receipt: DomainEvidenceProviderExternalPayloadReceipt,
    pub materialized_payload_digest: String,
    pub normalization: DomainEvidenceProviderNormalization,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum DomainEvidenceProviderExternalPayloadNormalizationError {
    #[error("external payload receipt refused: {0}")]
    Receipt(#[from] DomainEvidenceProviderExternalPayloadError),
    #[error("provider normalization refused: {0}")]
    Normalization(#[from] DomainEvidenceProviderNormalizationError),
    #[error("cannot canonicalize materialized payload: {0}")]
    Canonical(String),
    #[error(
        "materialized payload digest {observed} does not match receipt payload digest {expected}"
    )]
    PayloadDigestMismatch { expected: String, observed: String },
    #[error(
        "materialized canonical JSON is {observed} bytes but receipt declares {expected} bytes"
    )]
    PayloadByteLengthMismatch { expected: u64, observed: u64 },
    #[error(
        "materialized request digest {observed:?} does not match receipt request digest {expected}"
    )]
    RequestDigestMismatch {
        expected: String,
        observed: Option<String>,
    },
}

/// Verify a caller materialization against the receipt, then normalize it without external I/O.
pub fn normalize_domain_evidence_provider_external_payload(
    request: &DomainEvidenceProviderExternalPayloadNormalizationRequest,
) -> Result<
    DomainEvidenceProviderExternalPayloadNormalization,
    DomainEvidenceProviderExternalPayloadNormalizationError,
> {
    let receipt = record_domain_evidence_provider_external_payload(&request.receipt)?;
    let materialized_bytes = serde_json::to_vec(&request.payload).map_err(|error| {
        DomainEvidenceProviderExternalPayloadNormalizationError::Canonical(error.to_string())
    })?;
    let materialized_payload_digest = ContentHash::of_value(&request.payload)
        .map_err(|error| {
            DomainEvidenceProviderExternalPayloadNormalizationError::Canonical(error.to_string())
        })?
        .to_string();
    if materialized_payload_digest != receipt.payload_digest {
        return Err(
            DomainEvidenceProviderExternalPayloadNormalizationError::PayloadDigestMismatch {
                expected: receipt.payload_digest,
                observed: materialized_payload_digest,
            },
        );
    }
    if materialized_bytes.len() as u64 != receipt.byte_length {
        return Err(
            DomainEvidenceProviderExternalPayloadNormalizationError::PayloadByteLengthMismatch {
                expected: receipt.byte_length,
                observed: materialized_bytes.len() as u64,
            },
        );
    }
    let materialized_request_digest = request
        .request
        .as_ref()
        .map(|value| {
            ContentHash::of_value(value)
                .map(|digest| digest.to_string())
                .map_err(|error| {
                    DomainEvidenceProviderExternalPayloadNormalizationError::Canonical(
                        error.to_string(),
                    )
                })
        })
        .transpose()?;
    if let Some(expected_request_digest) = receipt.request_digest.as_ref() {
        if materialized_request_digest.as_deref() != Some(expected_request_digest.as_str()) {
            return Err(
                DomainEvidenceProviderExternalPayloadNormalizationError::RequestDigestMismatch {
                    expected: expected_request_digest.clone(),
                    observed: materialized_request_digest,
                },
            );
        }
    }

    let mut parent_digests = request.parent_digests.clone();
    parent_digests.push(receipt.receipt_digest.clone());
    let normalized =
        normalize_domain_evidence_provider(&DomainEvidenceProviderNormalizationRequest {
            group_id: receipt.group_id.clone(),
            domains: receipt.domains.clone(),
            subject_id: receipt.subject_id.clone(),
            source_tool: receipt.source_tool.clone(),
            connector_kind: receipt.connector_kind.clone(),
            provider: receipt.provider.clone(),
            payload: request.payload.clone(),
            request: request.request.clone(),
            outcome: request.outcome.clone(),
            claim_posture: request.claim_posture.clone(),
            parent_digests,
            source_plan_digest: request.source_plan_digest.clone(),
        })?;
    Ok(DomainEvidenceProviderExternalPayloadNormalization {
        schema: DOMAIN_EVIDENCE_PROVIDER_EXTERNAL_PAYLOAD_NORMALIZATION_SCHEMA.into(),
        workflow: DOMAIN_EVIDENCE_PROVIDER_EXTERNAL_PAYLOAD_NORMALIZATION_WORKFLOW.into(),
        receipt,
        materialized_payload_digest,
        normalization: normalized,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request() -> DomainEvidenceProviderExternalPayloadNormalizationRequest {
        let payload = json!({"records": [{"id": "pmid:1", "title": "opaque"}]});
        let payload_digest = ContentHash::of_value(&payload).unwrap().to_string();
        let byte_length = serde_json::to_vec(&payload).unwrap().len() as u64;
        DomainEvidenceProviderExternalPayloadNormalizationRequest {
            receipt: DomainEvidenceProviderExternalPayloadReceiptRequest {
                group_id: "biological_domains".into(),
                domains: vec!["oncology".into()],
                subject_id: "subject-1".into(),
                source_tool: "literature_bind_check".into(),
                provider: "pubmed".into(),
                connector_kind: "literature".into(),
                handoff_digest: "a".repeat(64),
                transfer_id: "transfer-1".into(),
                payload_digest,
                byte_length,
                storage_backend: "object_store".into(),
                locator_kind: "opaque".into(),
                locator: "store://caller/pubmed/objects/1".into(),
                content_type: Some("application/json".into()),
                content_encoding: None,
                request_digest: None,
                parent_digests: vec![],
                availability: "available".into(),
                retention: "durable".into(),
                attempt_id: None,
            },
            payload,
            request: None,
            outcome: "observed".into(),
            claim_posture: default_claim_posture(),
            parent_digests: vec![],
            source_plan_digest: None,
        }
    }

    #[test]
    fn bridge_requires_exact_materialized_digest_and_parents_receipt() {
        let bridged = normalize_domain_evidence_provider_external_payload(&request()).unwrap();
        assert_eq!(
            bridged.receipt.payload_digest,
            bridged.materialized_payload_digest
        );
        assert_eq!(
            bridged.normalization.payload_digest,
            bridged.materialized_payload_digest
        );
        assert_eq!(bridged.normalization.outcome, "observed");
        assert!(bridged.normalization.intake_arguments["parent_digests"]
            .as_array()
            .unwrap()
            .iter()
            .any(|parent| parent == &json!(bridged.receipt.receipt_digest)));
    }

    #[test]
    fn bridge_refuses_materialization_drift_before_normalization() {
        let mut request = request();
        request.payload = json!({"records": [{"id": "pmid:drift"}]});
        assert!(matches!(
            normalize_domain_evidence_provider_external_payload(&request),
            Err(
                DomainEvidenceProviderExternalPayloadNormalizationError::PayloadDigestMismatch { .. }
            )
        ));
    }

    #[test]
    fn bridge_binds_a_declared_receipt_request_digest_before_normalization() {
        let mut request = request();
        let materialized_request = json!({"query": "oncology"});
        request.receipt.request_digest = Some(
            ContentHash::of_value(&materialized_request)
                .unwrap()
                .to_string(),
        );
        request.request = Some(materialized_request);
        let bridged = normalize_domain_evidence_provider_external_payload(&request).unwrap();
        assert_eq!(
            bridged.normalization.request_digest,
            request.receipt.request_digest
        );

        request.request = Some(json!({"query": "different"}));
        assert!(matches!(
            normalize_domain_evidence_provider_external_payload(&request),
            Err(
                DomainEvidenceProviderExternalPayloadNormalizationError::RequestDigestMismatch { .. }
            )
        ));
    }
}
