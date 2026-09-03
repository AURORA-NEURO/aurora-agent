//! Caller-supplied execution evidence for external provider payload transfers.
//!
//! The core never executes a transfer. It can, however, retain a bounded caller observation and
//! compare the observation with a receipt already present in the local registry projection. This
//! keeps transfer evidence, provider authenticity, payload inspection, and readiness as separate
//! claims instead of collapsing them into a single success boolean.

use crate::domain_evidence_provider_external::{
    DomainEvidenceProviderExternalPayloadReceipt,
    DomainEvidenceProviderExternalPayloadReceiptRequest,
};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::BTreeMap;

pub const DOMAIN_EVIDENCE_PROVIDER_EXTERNAL_PAYLOAD_EXECUTION_SCHEMA: &str =
    "bioprism-devplat-domain-evidence-provider-external-payload-execution-evidence/0.1";
pub const DOMAIN_EVIDENCE_PROVIDER_EXTERNAL_PAYLOAD_EXECUTION_WORKFLOW: &str =
    "domain_evidence_provider_external_payload_execution_evidence";

const EXECUTION_STATUSES: &[&str] = &[
    "submitted",
    "transferred",
    "partial",
    "refused",
    "error",
    "unknown",
];

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct DomainEvidenceProviderExternalPayloadExecutionEvidenceRequest {
    #[serde(flatten)]
    pub receipt: DomainEvidenceProviderExternalPayloadReceiptRequest,
    pub expected_receipt_digest: String,
    pub execution_status: String,
    pub executor_id: String,
    #[serde(default)]
    pub observed_payload_digest: Option<String>,
    #[serde(default)]
    pub observed_byte_length: Option<u64>,
    #[serde(default)]
    pub locator_opened: bool,
    #[serde(default)]
    pub observation_digest: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct DomainEvidenceProviderExternalPayloadExecutionEvidence {
    pub schema: String,
    pub workflow: String,
    pub evidence_status: String,
    pub group_id: String,
    pub domains: Vec<String>,
    pub subject_id: String,
    pub source_tool: String,
    pub provider: String,
    pub connector_kind: String,
    pub expected_receipt_digest: String,
    pub retained_receipt_digest: Option<String>,
    pub observed_receipt_digest: String,
    pub execution_status: String,
    pub executor_id: String,
    pub observed_payload_digest: Option<String>,
    pub observed_byte_length: Option<u64>,
    pub locator_opened: bool,
    pub observation_digest: Option<String>,
    pub receipt: DomainEvidenceProviderExternalPayloadReceipt,
    pub matches: BTreeMap<String, bool>,
    pub differences: Vec<String>,
    pub evidence_digest: String,
    pub guarantees: Vec<String>,
    pub limitations: Vec<String>,
}

fn digest(name: &str, value: &str) -> Result<String, String> {
    if value.len() != 64
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    {
        return Err(format!("{name} must be a lowercase SHA-256 digest"));
    }
    ContentHash::parse(value.to_owned()).map_err(|error| format!("{name} is invalid: {error}"))?;
    Ok(value.to_owned())
}

fn text(name: &str, value: &str) -> Result<String, String> {
    if value.trim().is_empty() || value.len() > 512 || value.chars().any(char::is_control) {
        return Err(format!(
            "{name} must be non-empty text of at most 512 bytes"
        ));
    }
    Ok(value.to_owned())
}

fn optional_digest(name: &str, value: &Option<String>) -> Result<Option<String>, String> {
    value
        .as_deref()
        .map(|value| digest(name, value))
        .transpose()
}

fn canonical_digest(value: &serde_json::Value) -> Result<String, String> {
    ContentHash::of_value(value)
        .map(|digest| digest.to_string())
        .map_err(|error| error.to_string())
}

fn canonical_receipt(
    receipt: DomainEvidenceProviderExternalPayloadReceipt,
) -> Result<DomainEvidenceProviderExternalPayloadReceipt, String> {
    let request = DomainEvidenceProviderExternalPayloadReceiptRequest {
        group_id: receipt.group_id.clone(),
        domains: receipt.domains.clone(),
        subject_id: receipt.subject_id.clone(),
        source_tool: receipt.source_tool.clone(),
        provider: receipt.provider.clone(),
        connector_kind: receipt.connector_kind.clone(),
        handoff_digest: receipt.handoff_digest.clone(),
        transfer_id: receipt.transfer_id.clone(),
        payload_digest: receipt.payload_digest.clone(),
        byte_length: receipt.byte_length,
        storage_backend: receipt.storage_backend.clone(),
        locator_kind: receipt.locator_kind.clone(),
        locator: receipt.locator.clone(),
        content_type: receipt.content_type.clone(),
        content_encoding: receipt.content_encoding.clone(),
        request_digest: receipt.request_digest.clone(),
        parent_digests: receipt.parent_digests.clone(),
        availability: receipt.availability.clone(),
        retention: receipt.retention.clone(),
        attempt_id: receipt.attempt_id.clone(),
    };
    let canonical =
        crate::domain_evidence_provider_external::record_domain_evidence_provider_external_payload(
            &request,
        )
        .map_err(|error| format!("receipt is invalid: {error}"))?;
    if canonical != receipt {
        return Err("receipt is not the canonical digest-bound receipt for its metadata".into());
    }
    Ok(canonical)
}

/// Compare caller-reported transfer observations with a retained external receipt.
pub fn audit_domain_evidence_provider_external_payload_execution(
    receipt: DomainEvidenceProviderExternalPayloadReceipt,
    retained_receipt: Option<DomainEvidenceProviderExternalPayloadReceipt>,
    request: &DomainEvidenceProviderExternalPayloadExecutionEvidenceRequest,
) -> Result<DomainEvidenceProviderExternalPayloadExecutionEvidence, String> {
    let receipt = canonical_receipt(receipt)?;
    let requested_receipt =
        crate::domain_evidence_provider_external::record_domain_evidence_provider_external_payload(
            &request.receipt,
        )
        .map_err(|error| format!("request receipt is invalid: {error}"))?;
    if requested_receipt != receipt {
        return Err("request receipt does not match the canonical supplied receipt".into());
    }
    let retained_receipt = retained_receipt.map(canonical_receipt).transpose()?;
    let expected_receipt_digest =
        digest("expected_receipt_digest", &request.expected_receipt_digest)?;
    let execution_status = text("execution_status", &request.execution_status)?;
    if !EXECUTION_STATUSES.contains(&execution_status.as_str()) {
        return Err(format!(
            "execution_status must be one of {}",
            EXECUTION_STATUSES.join(", ")
        ));
    }
    let executor_id = text("executor_id", &request.executor_id)?;
    if executor_id != executor_id.trim() {
        return Err("executor_id must not contain surrounding whitespace".into());
    }
    let observed_payload_digest =
        optional_digest("observed_payload_digest", &request.observed_payload_digest)?;
    if let Some(byte_length) = request.observed_byte_length {
        if !(1..=64 * 1024 * 1024 * 1024u64).contains(&byte_length) {
            return Err("observed_byte_length must be between 1 and 68719476736".into());
        }
    }
    let observation_digest = optional_digest("observation_digest", &request.observation_digest)?;

    let mut matches: BTreeMap<String, bool> = BTreeMap::new();
    let mut differences: Vec<String> = Vec::new();
    let retained_receipt_digest = retained_receipt
        .as_ref()
        .map(|retained| retained.receipt_digest.clone());
    let receipt_present = retained_receipt.is_some();
    matches.insert("receipt_present".into(), receipt_present);
    let expected_matches = receipt.receipt_digest == expected_receipt_digest;
    matches.insert("expected_receipt_digest".into(), expected_matches);
    if !expected_matches {
        differences.push("expected_receipt_digest".into());
    }

    let retained_matches = if let Some(retained) = retained_receipt.as_ref() {
        let identity = [
            (
                "receipt_digest",
                retained.receipt_digest == receipt.receipt_digest,
            ),
            (
                "handoff_digest",
                retained.handoff_digest == receipt.handoff_digest,
            ),
            (
                "payload_digest",
                retained.payload_digest == receipt.payload_digest,
            ),
            ("byte_length", retained.byte_length == receipt.byte_length),
            ("group_id", retained.group_id == receipt.group_id),
            ("domains", retained.domains == receipt.domains),
            ("subject_id", retained.subject_id == receipt.subject_id),
            ("source_tool", retained.source_tool == receipt.source_tool),
            ("provider", retained.provider == receipt.provider),
            (
                "connector_kind",
                retained.connector_kind == receipt.connector_kind,
            ),
        ];
        for (name, matched) in identity {
            matches.insert(name.into(), matched);
            if !matched {
                differences.push(name.into());
            }
        }
        identity.iter().all(|(_, matched)| *matched)
    } else {
        differences.push("receipt_not_retained".into());
        false
    };

    let payload_match = match observed_payload_digest.as_ref() {
        Some(observed) => {
            let matched = observed == &receipt.payload_digest;
            matches.insert("observed_payload_digest".into(), matched);
            if !matched {
                differences.push("observed_payload_digest".into());
            }
            matched
        }
        None => {
            matches.insert("observed_payload_digest".into(), false);
            differences.push("observed_payload_digest_not_supplied".into());
            false
        }
    };
    let byte_match = match request.observed_byte_length {
        Some(observed) => {
            let matched = observed == receipt.byte_length;
            matches.insert("observed_byte_length".into(), matched);
            if !matched {
                differences.push("observed_byte_length".into());
            }
            matched
        }
        None => {
            matches.insert("observed_byte_length".into(), false);
            differences.push("observed_byte_length_not_supplied".into());
            false
        }
    };
    let evidence_status = if !receipt_present {
        "orphaned"
    } else if !expected_matches
        || !retained_matches
        || differences.iter().any(|difference| {
            difference == "observed_payload_digest" || difference == "observed_byte_length"
        })
    {
        "mismatch"
    } else if !payload_match || !byte_match {
        "partial"
    } else {
        "matched"
    };

    let mut unsigned = serde_json::to_value(json!({
        "schema": DOMAIN_EVIDENCE_PROVIDER_EXTERNAL_PAYLOAD_EXECUTION_SCHEMA,
        "workflow": DOMAIN_EVIDENCE_PROVIDER_EXTERNAL_PAYLOAD_EXECUTION_WORKFLOW,
        "evidence_status": evidence_status,
        "group_id": receipt.group_id,
        "domains": receipt.domains,
        "subject_id": receipt.subject_id,
        "source_tool": receipt.source_tool,
        "provider": receipt.provider,
        "connector_kind": receipt.connector_kind,
        "expected_receipt_digest": expected_receipt_digest,
        "retained_receipt_digest": retained_receipt_digest,
        "observed_receipt_digest": receipt.receipt_digest,
        "execution_status": execution_status,
        "executor_id": executor_id,
        "observed_payload_digest": observed_payload_digest,
        "observed_byte_length": request.observed_byte_length,
        "locator_opened": request.locator_opened,
        "observation_digest": observation_digest,
        "receipt": receipt,
        "matches": matches,
        "differences": differences,
        "guarantees": [
            "caller-supplied execution observations are compared with a retained receipt when present",
            "observed payload digest and byte length remain independently visible",
            "the core performs no transfer, locator, provider, credential, or payload operation"
        ],
        "limitations": [
            "caller execution status and locator_opened are assertions, not cryptographic attestations",
            "matching receipt metadata does not prove provider authenticity, payload authenticity, or transfer causality",
            "scientific, clinical, provenance, regulatory, release, and readiness validity remain unclaimed"
        ]
    }))
    .map_err(|error| error.to_string())?;
    let evidence_digest = canonical_digest(&unsigned)?;
    unsigned["evidence_digest"] = json!(evidence_digest);
    serde_json::from_value(unsigned).map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain_evidence_provider_external::record_domain_evidence_provider_external_payload;

    fn receipt_request() -> DomainEvidenceProviderExternalPayloadReceiptRequest {
        DomainEvidenceProviderExternalPayloadReceiptRequest {
            group_id: "biological_domains".into(),
            domains: vec!["oncology".into()],
            subject_id: "execution-subject".into(),
            source_tool: "literature_bind_check".into(),
            provider: "pubmed".into(),
            connector_kind: "literature".into(),
            handoff_digest: "a".repeat(64),
            transfer_id: "transfer-execution-1".into(),
            payload_digest: "b".repeat(64),
            byte_length: 4096,
            storage_backend: "object_store".into(),
            locator_kind: "opaque".into(),
            locator: "store://caller/pubmed/execution-1".into(),
            content_type: None,
            content_encoding: None,
            request_digest: None,
            parent_digests: vec![],
            availability: "available".into(),
            retention: "durable".into(),
            attempt_id: None,
        }
    }

    fn evidence(
        receipt: &DomainEvidenceProviderExternalPayloadReceipt,
    ) -> DomainEvidenceProviderExternalPayloadExecutionEvidenceRequest {
        DomainEvidenceProviderExternalPayloadExecutionEvidenceRequest {
            receipt: receipt_request(),
            expected_receipt_digest: receipt.receipt_digest.clone(),
            execution_status: "transferred".into(),
            executor_id: "caller-transfer-worker".into(),
            observed_payload_digest: Some(receipt.payload_digest.clone()),
            observed_byte_length: Some(receipt.byte_length),
            locator_opened: true,
            observation_digest: Some("c".repeat(64)),
        }
    }

    #[test]
    fn execution_evidence_distinguishes_matched_partial_mismatch_and_orphaned_receipts() {
        let receipt = record_domain_evidence_provider_external_payload(&receipt_request()).unwrap();
        let matched = audit_domain_evidence_provider_external_payload_execution(
            receipt.clone(),
            Some(receipt.clone()),
            &evidence(&receipt),
        )
        .unwrap();
        assert_eq!(matched.evidence_status, "matched");
        let mut matched_value = serde_json::to_value(&matched).unwrap();
        matched_value["unexpected"] = json!(true);
        assert!(
            serde_json::from_value::<DomainEvidenceProviderExternalPayloadExecutionEvidence>(
                matched_value
            )
            .is_err()
        );
        let mut partial_request = evidence(&receipt);
        partial_request.observed_byte_length = None;
        let partial = audit_domain_evidence_provider_external_payload_execution(
            receipt.clone(),
            Some(receipt.clone()),
            &partial_request,
        )
        .unwrap();
        assert_eq!(partial.evidence_status, "partial");
        let mut mismatch_request = evidence(&receipt);
        mismatch_request.observed_payload_digest = Some("d".repeat(64));
        let mismatch = audit_domain_evidence_provider_external_payload_execution(
            receipt.clone(),
            Some(receipt.clone()),
            &mismatch_request,
        )
        .unwrap();
        assert_eq!(mismatch.evidence_status, "mismatch");
        let orphaned = audit_domain_evidence_provider_external_payload_execution(
            receipt.clone(),
            None,
            &evidence(&receipt),
        )
        .unwrap();
        assert_eq!(orphaned.evidence_status, "orphaned");
    }

    #[test]
    fn execution_evidence_rejects_forged_receipt_objects_and_identity_whitespace() {
        let receipt = record_domain_evidence_provider_external_payload(&receipt_request()).unwrap();
        let mut forged = receipt.clone();
        forged.payload_digest = "d".repeat(64);
        let error = audit_domain_evidence_provider_external_payload_execution(
            forged,
            Some(receipt.clone()),
            &evidence(&receipt),
        )
        .expect_err("receipt metadata must be re-canonicalized before comparison");
        assert!(error.contains("canonical digest-bound receipt"));

        let mut invalid = evidence(&receipt);
        invalid.executor_id = " transfer-worker".into();
        let error = audit_domain_evidence_provider_external_payload_execution(
            receipt.clone(),
            Some(receipt),
            &invalid,
        )
        .expect_err("executor identity whitespace must be rejected");
        assert!(error.contains("executor_id"));
    }

    #[test]
    fn execution_evidence_binds_flattened_request_receipt_to_supplied_receipt() {
        let receipt = record_domain_evidence_provider_external_payload(&receipt_request()).unwrap();
        let mut invalid = evidence(&receipt);
        invalid.receipt.payload_digest = "d".repeat(64);

        let error =
            audit_domain_evidence_provider_external_payload_execution(receipt, None, &invalid)
                .expect_err("request receipt fields must bind to the supplied receipt");
        assert!(error.contains("does not match the canonical supplied receipt"));
    }
}
