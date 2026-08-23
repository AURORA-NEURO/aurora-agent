//! Registry-backed lineage audit for external provider payload receipts.
//!
//! A syntactically valid handoff digest is not enough to establish that a receipt belongs to the
//! retained connector declaration. This module compares the receipt to an already validated
//! handoff artifact supplied by the local registry projection. It performs no provider, network,
//! storage, credential, or payload operation.

use crate::domain_evidence_provider_external::DomainEvidenceProviderExternalPayloadReceipt;
use crate::domain_evidence_provider_handoff::DomainEvidenceProviderHandoff;
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::BTreeMap;

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

/// Compare a receipt with the retained handoff that claims its connector scope.
pub fn audit_domain_evidence_provider_external_payload_lineage(
    receipt: DomainEvidenceProviderExternalPayloadReceipt,
    handoff: Option<DomainEvidenceProviderHandoff>,
) -> Result<DomainEvidenceProviderExternalPayloadLineageAudit, String> {
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
    use crate::domain_evidence_provider_external::DomainEvidenceProviderExternalPayloadReceipt;

    fn receipt() -> DomainEvidenceProviderExternalPayloadReceipt {
        DomainEvidenceProviderExternalPayloadReceipt {
            schema: "receipt".into(),
            workflow: "receipt".into(),
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
            receipt_digest: "c".repeat(64),
            execution: "not_started".into(),
            readiness_claimed: false,
            guarantees: vec![],
            limitations: vec![],
        }
    }

    #[test]
    fn lineage_distinguishes_matched_partial_and_orphaned_handoffs() {
        let audit =
            audit_domain_evidence_provider_external_payload_lineage(receipt(), None).unwrap();
        assert_eq!(audit.lineage_status, "orphaned");
        assert_eq!(audit.payload_binding_status, "not_available");
        let mut handoff = DomainEvidenceProviderHandoff {
            schema: "handoff".into(), workflow: "handoff".into(), group_id: "biological_domains".into(),
            domains: vec!["oncology".into()], subject_id: "subject-1".into(), source_tool: "literature_bind_check".into(),
            provider: "pubmed".into(), connector_kind: "literature".into(), status: "prepared".into(),
            manifest: serde_json::from_value(json!({
                "schema": "manifest", "connector_id": "caller", "version": "1", "provider": "pubmed",
                "connector_kind": "literature", "domains": ["oncology"], "capabilities": ["retain"],
                "transport": "caller_managed", "auth_posture": {"status": "unknown", "does_not_claim": ["auth"]}
            })).unwrap(),
            manifest_digest: "d".repeat(64), request_digest: None, payload_digest: None, source_plan_digest: None,
            parent_digests: vec![], attempt_id: None, handoff_digest: "a".repeat(64), execution: "not_started".into(),
            readiness_claimed: false, guarantees: vec![], limitations: vec![],
        };
        let partial = audit_domain_evidence_provider_external_payload_lineage(
            receipt(),
            Some(handoff.clone()),
        )
        .unwrap();
        assert_eq!(partial.lineage_status, "partial");
        handoff.payload_digest = Some("b".repeat(64));
        let matched =
            audit_domain_evidence_provider_external_payload_lineage(receipt(), Some(handoff))
                .unwrap();
        assert_eq!(matched.lineage_status, "matched");
        assert!(matched.matches["payload_digest"]);
    }
}
