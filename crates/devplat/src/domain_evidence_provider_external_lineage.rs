//! Registry-backed lineage audit for external provider payload receipts.
//!
//! A syntactically valid handoff digest is not enough to establish that a receipt belongs to the
//! retained connector declaration. This module compares the receipt to an already validated
//! handoff artifact supplied by the local registry projection. It performs no provider, network,
//! storage, credential, or payload operation.

use crate::domain_evidence_provider_external::{
    record_domain_evidence_provider_external_payload, DomainEvidenceProviderExternalPayloadReceipt,
    DomainEvidenceProviderExternalPayloadReceiptRequest,
};
use crate::domain_evidence_provider_handoff::{
    handoff_domain_evidence_provider, DomainEvidenceProviderHandoff,
    DomainEvidenceProviderHandoffRequest,
};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::{BTreeMap, BTreeSet};

pub const DOMAIN_EVIDENCE_PROVIDER_EXTERNAL_PAYLOAD_LINEAGE_SCHEMA: &str =
    "bioprism-devplat-domain-evidence-provider-external-payload-lineage/0.1";
pub const DOMAIN_EVIDENCE_PROVIDER_EXTERNAL_PAYLOAD_LINEAGE_WORKFLOW: &str =
    "domain_evidence_provider_external_payload_lineage_audit";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct DomainEvidenceProviderExternalPayloadLineageAuditRequest {
    #[serde(flatten)]
    pub receipt: crate::domain_evidence_provider_external::DomainEvidenceProviderExternalPayloadReceiptRequest,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DomainEvidenceProviderExternalPayloadLineageAudit {
    pub schema: String,
    pub workflow: String,
    pub lineage_status: String,
    pub group_id: String,
    pub domains: Vec<String>,
    pub subject_id: String,
    pub source_tool: String,
    pub provider: String,
    pub connector_kind: String,
    pub receipt: DomainEvidenceProviderExternalPayloadReceipt,
    pub handoff: Option<DomainEvidenceProviderHandoff>,
    pub matches: BTreeMap<String, bool>,
    pub differences: Vec<String>,
    pub payload_binding_status: String,
    pub lineage_digest: String,
    pub guarantees: Vec<String>,
    pub limitations: Vec<String>,
}

fn canonical_digest(value: &serde_json::Value) -> Result<String, String> {
    ContentHash::of_value(value)
        .map(|digest| digest.to_string())
        .map_err(|error| error.to_string())
}

fn valid_text(name: &str, value: &str) -> Result<(), String> {
    if value.trim().is_empty() || value.len() > 512 || value.chars().any(char::is_control) {
        return Err(format!("{name} must be bounded visible text"));
    }
    if value != value.trim() {
        return Err(format!("{name} must not contain surrounding whitespace"));
    }
    Ok(())
}

fn valid_digest(name: &str, value: &str) -> Result<(), String> {
    if value.len() != 64
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        || ContentHash::parse(value.to_owned()).is_err()
    {
        return Err(format!("{name} must be a lowercase SHA-256 digest"));
    }
    Ok(())
}

fn validate_receipt(receipt: &DomainEvidenceProviderExternalPayloadReceipt) -> Result<(), String> {
    for (name, value) in [
        ("receipt.group_id", &receipt.group_id),
        ("receipt.subject_id", &receipt.subject_id),
        ("receipt.source_tool", &receipt.source_tool),
        ("receipt.provider", &receipt.provider),
        ("receipt.connector_kind", &receipt.connector_kind),
        ("receipt.transfer_id", &receipt.transfer_id),
    ] {
        valid_text(name, value)?;
    }
    let mut domains = BTreeSet::new();
    for domain in &receipt.domains {
        valid_text("receipt.domain", domain)?;
        if !domains.insert(domain.to_ascii_lowercase()) {
            return Err("receipt.domains must not contain duplicate identities".into());
        }
    }
    for (name, value) in [
        ("receipt.handoff_digest", &receipt.handoff_digest),
        ("receipt.payload_digest", &receipt.payload_digest),
        ("receipt.receipt_digest", &receipt.receipt_digest),
    ] {
        valid_digest(name, value)?;
    }
    for (name, value) in [
        ("receipt.request_digest", receipt.request_digest.as_ref()),
        ("receipt.attempt_id", receipt.attempt_id.as_ref()),
    ] {
        if let Some(value) = value {
            if name.ends_with("digest") {
                valid_digest(name, value)?;
            } else {
                valid_text(name, value)?;
            }
        }
    }
    for (index, parent) in receipt.parent_digests.iter().enumerate() {
        valid_digest(&format!("receipt.parent_digests[{index}]"), parent)?;
    }
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
    let canonical = record_domain_evidence_provider_external_payload(&request)
        .map_err(|error| format!("receipt is not canonical: {error}"))?;
    if canonical != *receipt {
        return Err("receipt is not the canonical digest-bound receipt for its metadata".into());
    }
    Ok(())
}

fn validate_handoff(handoff: &DomainEvidenceProviderHandoff) -> Result<(), String> {
    for (name, value) in [
        ("handoff.group_id", &handoff.group_id),
        ("handoff.subject_id", &handoff.subject_id),
        ("handoff.source_tool", &handoff.source_tool),
        ("handoff.provider", &handoff.provider),
        ("handoff.connector_kind", &handoff.connector_kind),
    ] {
        valid_text(name, value)?;
    }
    for (name, value) in [
        ("handoff.manifest_digest", &handoff.manifest_digest),
        ("handoff.handoff_digest", &handoff.handoff_digest),
    ] {
        valid_digest(name, value)?;
    }
    for (name, value) in [
        ("handoff.request_digest", handoff.request_digest.as_ref()),
        ("handoff.payload_digest", handoff.payload_digest.as_ref()),
        (
            "handoff.source_plan_digest",
            handoff.source_plan_digest.as_ref(),
        ),
    ] {
        if let Some(value) = value {
            valid_digest(name, value)?;
        }
    }
    for (index, parent) in handoff.parent_digests.iter().enumerate() {
        valid_digest(&format!("handoff.parent_digests[{index}]"), parent)?;
    }
    if let Some(attempt_id) = handoff.attempt_id.as_ref() {
        valid_text("handoff.attempt_id", attempt_id)?;
    }
    let request = DomainEvidenceProviderHandoffRequest {
        group_id: handoff.group_id.clone(),
        domains: handoff.domains.clone(),
        subject_id: handoff.subject_id.clone(),
        source_tool: handoff.source_tool.clone(),
        provider: handoff.provider.clone(),
        connector_kind: handoff.connector_kind.clone(),
        manifest: handoff.manifest.clone(),
        status: handoff.status.clone(),
        request_digest: handoff.request_digest.clone(),
        payload_digest: handoff.payload_digest.clone(),
        source_plan_digest: handoff.source_plan_digest.clone(),
        parent_digests: handoff.parent_digests.clone(),
        attempt_id: handoff.attempt_id.clone(),
    };
    let canonical = handoff_domain_evidence_provider(&request)
        .map_err(|error| format!("handoff is not canonical: {error}"))?;
    if canonical != *handoff {
        return Err("handoff is not the canonical digest-bound handoff for its metadata".into());
    }
    Ok(())
}

/// Compare a receipt with the retained handoff that claims its connector scope.
pub fn audit_domain_evidence_provider_external_payload_lineage(
    receipt: DomainEvidenceProviderExternalPayloadReceipt,
    handoff: Option<DomainEvidenceProviderHandoff>,
) -> Result<DomainEvidenceProviderExternalPayloadLineageAudit, String> {
    validate_receipt(&receipt)?;
    if let Some(handoff) = handoff.as_ref() {
        validate_handoff(handoff)?;
    }
    let mut matches: BTreeMap<String, bool> = BTreeMap::new();
    let handoff_present = handoff.is_some();
    matches.insert("handoff_present".into(), handoff_present);
    let mut differences: Vec<String> = Vec::new();
    let (payload_binding_status, scope_matches) = if let Some(handoff) = handoff.as_ref() {
        let handoff_digest = handoff.handoff_digest == receipt.handoff_digest;
        let group_id = handoff.group_id == receipt.group_id;
        let domains = handoff.domains == receipt.domains;
        let subject_id = handoff.subject_id == receipt.subject_id;
        let source_tool = handoff.source_tool == receipt.source_tool;
        let provider = handoff.provider == receipt.provider;
        let connector_kind = handoff.connector_kind == receipt.connector_kind;
        matches.insert("handoff_digest".into(), handoff_digest);
        matches.insert("group_id".into(), group_id);
        matches.insert("domains".into(), domains);
        matches.insert("subject_id".into(), subject_id);
        matches.insert("source_tool".into(), source_tool);
        matches.insert("provider".into(), provider);
        matches.insert("connector_kind".into(), connector_kind);
        for (name, matched) in [
            ("handoff_digest", handoff_digest),
            ("group_id", group_id),
            ("domains", domains),
            ("subject_id", subject_id),
            ("source_tool", source_tool),
            ("provider", provider),
            ("connector_kind", connector_kind),
        ] {
            if !matched {
                differences.push(name.into());
            }
        }
        let payload_binding_status = match handoff.payload_digest.as_ref() {
            Some(payload_digest) if payload_digest == &receipt.payload_digest => {
                matches.insert("payload_digest".into(), true);
                "matched"
            }
            Some(_) => {
                matches.insert("payload_digest".into(), false);
                differences.push("payload_digest".into());
                "mismatch"
            }
            None => {
                matches.insert("payload_digest_declared".into(), false);
                differences.push("payload_digest_not_declared".into());
                "not_declared"
            }
        };
        (
            payload_binding_status,
            handoff_digest
                && group_id
                && domains
                && subject_id
                && source_tool
                && provider
                && connector_kind,
        )
    } else {
        differences.push("handoff_not_retained".into());
        matches.insert("payload_digest_declared".into(), false);
        ("not_available", false)
    };
    let lineage_status = if !handoff_present {
        "orphaned"
    } else if !scope_matches || payload_binding_status == "mismatch" {
        "mismatch"
    } else if payload_binding_status == "not_declared" {
        "partial"
    } else {
        "matched"
    };
    let mut unsigned = serde_json::to_value(json!({
        "schema": DOMAIN_EVIDENCE_PROVIDER_EXTERNAL_PAYLOAD_LINEAGE_SCHEMA,
        "workflow": DOMAIN_EVIDENCE_PROVIDER_EXTERNAL_PAYLOAD_LINEAGE_WORKFLOW,
        "lineage_status": lineage_status,
        "group_id": receipt.group_id,
        "domains": receipt.domains,
        "subject_id": receipt.subject_id,
        "source_tool": receipt.source_tool,
        "provider": receipt.provider,
        "connector_kind": receipt.connector_kind,
        "receipt": receipt,
        "handoff": handoff,
        "matches": matches,
        "differences": differences,
        "payload_binding_status": payload_binding_status,
        "guarantees": [
            "receipt identity is compared with a retained connector-handoff artifact",
            "scope and optional payload binding mismatches remain individually visible",
            "the audit performs no provider, store, locator, credential, or payload operation"
        ],
        "limitations": [
            "a matched handoff proves registry identity and declared scope, not provider authenticity or execution",
            "an undeclared handoff payload digest remains partial lineage rather than a payload match",
            "scientific, clinical, provenance, regulatory, and release validity remain unclaimed"
        ]
    }))
    .map_err(|error| error.to_string())?;
    let lineage_digest = canonical_digest(&unsigned)?;
    unsigned["lineage_digest"] = json!(lineage_digest);
    serde_json::from_value(unsigned).map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain_evidence_provider_external::DomainEvidenceProviderExternalPayloadReceiptRequest;
    use crate::domain_evidence_provider_handoff::{
        DomainEvidenceProviderAuthPosture, DomainEvidenceProviderConnectorManifest,
    };

    fn receipt_request() -> DomainEvidenceProviderExternalPayloadReceiptRequest {
        DomainEvidenceProviderExternalPayloadReceiptRequest {
            group_id: "biological_domains".into(),
            domains: vec!["oncology".into()],
            subject_id: "subject-1".into(),
            source_tool: "literature_bind_check".into(),
            provider: "pubmed".into(),
            connector_kind: "literature".into(),
            handoff_digest: "a".repeat(64),
            transfer_id: "transfer-1".into(),
            payload_digest: "b".repeat(64),
            byte_length: 1,
            storage_backend: "object_store".into(),
            locator_kind: "opaque".into(),
            locator: "store://object/1".into(),
            content_type: None,
            content_encoding: None,
            request_digest: None,
            parent_digests: vec![],
            availability: "available".into(),
            retention: "durable".into(),
            attempt_id: None,
        }
    }

    fn receipt() -> DomainEvidenceProviderExternalPayloadReceipt {
        receipt_for_handoff("a".repeat(64))
    }

    fn receipt_for_handoff(handoff_digest: String) -> DomainEvidenceProviderExternalPayloadReceipt {
        let mut request = receipt_request();
        request.handoff_digest = handoff_digest;
        crate::domain_evidence_provider_external::record_domain_evidence_provider_external_payload(
            &request,
        )
        .unwrap()
    }

    fn handoff(payload_digest: Option<String>) -> DomainEvidenceProviderHandoff {
        handoff_domain_evidence_provider(&DomainEvidenceProviderHandoffRequest {
            group_id: "biological_domains".into(),
            domains: vec!["oncology".into()],
            subject_id: "subject-1".into(),
            source_tool: "literature_bind_check".into(),
            provider: "pubmed".into(),
            connector_kind: "literature".into(),
            manifest: DomainEvidenceProviderConnectorManifest {
                schema: crate::domain_evidence_provider_handoff::
                    DOMAIN_EVIDENCE_PROVIDER_MANIFEST_SCHEMA
                    .into(),
                connector_id: "caller".into(),
                version: "1".into(),
                provider: "pubmed".into(),
                connector_kind: "literature".into(),
                domains: vec!["oncology".into()],
                capabilities: vec!["retain".into()],
                transport: "caller_managed".into(),
                auth_posture: DomainEvidenceProviderAuthPosture {
                    status: "unknown".into(),
                    secret_refs: vec![],
                    does_not_claim: vec!["provider authentication".into()],
                },
            },
            status: "prepared".into(),
            request_digest: None,
            payload_digest,
            source_plan_digest: None,
            parent_digests: vec![],
            attempt_id: None,
        })
        .unwrap()
    }

    #[test]
    fn lineage_distinguishes_matched_partial_and_orphaned_handoffs() {
        let audit =
            audit_domain_evidence_provider_external_payload_lineage(receipt(), None).unwrap();
        assert_eq!(audit.lineage_status, "orphaned");
        assert_eq!(audit.payload_binding_status, "not_available");
        let partial_handoff = handoff(None);
        let partial = audit_domain_evidence_provider_external_payload_lineage(
            receipt_for_handoff(partial_handoff.handoff_digest.clone()),
            Some(partial_handoff),
        )
        .unwrap();
        assert_eq!(partial.lineage_status, "partial");
        let matched_handoff = handoff(Some("b".repeat(64)));
        let matched = audit_domain_evidence_provider_external_payload_lineage(
            receipt_for_handoff(matched_handoff.handoff_digest.clone()),
            Some(matched_handoff),
        )
        .unwrap();
        assert_eq!(matched.lineage_status, "matched");
        assert!(matched.matches["payload_digest"]);
    }

    #[test]
    fn lineage_rejects_noncanonical_receipt_and_handoff_identity() {
        let mut invalid_receipt = receipt();
        invalid_receipt.payload_digest = "A".repeat(64);
        let error = audit_domain_evidence_provider_external_payload_lineage(invalid_receipt, None)
            .expect_err("uppercase receipt digests must be rejected");
        assert!(error.contains("receipt.payload_digest"));

        let mut invalid_handoff = handoff(None);
        invalid_handoff.subject_id = " subject-1".into();
        let error = audit_domain_evidence_provider_external_payload_lineage(
            receipt(),
            Some(invalid_handoff),
        )
        .expect_err("handoff identity whitespace must be rejected");
        assert!(error.contains("handoff.subject_id"));
    }

    #[test]
    fn lineage_rejects_digest_shaped_but_noncanonical_artifacts() {
        let mut forged_receipt = receipt();
        forged_receipt.receipt_digest = "e".repeat(64);
        let error = audit_domain_evidence_provider_external_payload_lineage(forged_receipt, None)
            .expect_err("receipt digest must bind to receipt metadata");
        assert!(error.contains("canonical digest-bound receipt"));

        let mut forged_handoff = handoff(None);
        forged_handoff.handoff_digest = "e".repeat(64);
        let error = audit_domain_evidence_provider_external_payload_lineage(
            receipt(),
            Some(forged_handoff),
        )
        .expect_err("handoff digest must bind to handoff metadata");
        assert!(error.contains("canonical digest-bound handoff"));
    }
}
