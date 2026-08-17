//! Digest-addressed caller evidence for concrete adapter execution.
//!
//! The adapter registry declares routes; the Python and native adapter layers perform the
//! actual parsing/conformance work. This module is the narrow handoff between those planes. It
//! retains what a caller observed about one selected adapter, including explicit refusal,
//! conformance, output identity, and semantic-loss posture, without making the MCP core execute
//! Python, import packages, open locators, or certify scientific meaning.

use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::BTreeSet;

pub const ADAPTER_EXECUTION_EVIDENCE_SCHEMA: &str =
    "bioprism-devplat-adapter-execution-evidence/0.1";
pub const ADAPTER_EXECUTION_EVIDENCE_WORKFLOW: &str = "adapter_execution_evidence";
pub const MAX_ADAPTER_EXECUTION_EVIDENCE_DOMAINS: usize = 64;
pub const MAX_ADAPTER_EXECUTION_EVIDENCE_LOSSES: usize = 128;
pub const MAX_ADAPTER_EXECUTION_EVIDENCE_PARENTS: usize = 128;
pub const MAX_ADAPTER_EXECUTION_EVIDENCE_TEXT_BYTES: usize = 512;
pub const MAX_ADAPTER_EXECUTION_EVIDENCE_BYTES: u64 = 68_719_476_736;
pub const MAX_ADAPTER_EXECUTION_EVIDENCE_ITEMS: u64 = 2_000_000;

const EXECUTION_STATUSES: &[&str] = &[
    "planned",
    "started",
    "succeeded",
    "partial",
    "refused",
    "failed",
    "unknown",
];
const CONFORMANCE_STATUSES: &[&str] = &["verified", "partial", "refused", "not_run", "unknown"];
const SEMANTIC_LOSS_STATUSES: &[&str] = &["lossless", "lossy", "unknown", "not_applicable"];
const LOSS_SEVERITIES: &[&str] = &["info", "warning", "blocking"];

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct AdapterExecutionLoss {
    pub kind: String,
    pub severity: String,
    pub detail: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct AdapterExecutionEvidenceRequest {
    pub group_id: String,
    pub domains: Vec<String>,
    pub subject_id: String,
    pub adapter_id: String,
    pub adapter_version: String,
    pub source_id: String,
    pub input_digest: String,
    #[serde(default)]
    pub output_digest: Option<String>,
    pub execution_status: String,
    pub conformance_status: String,
    pub semantic_loss_status: String,
    #[serde(default)]
    pub losses: Vec<AdapterExecutionLoss>,
    #[serde(default)]
    pub item_count: Option<u64>,
    #[serde(default)]
    pub byte_length: Option<u64>,
    #[serde(default)]
    pub error_code: Option<String>,
    #[serde(default)]
    pub parent_digests: Vec<String>,
    #[serde(default)]
    pub attempt_id: Option<String>,
}

fn text(field: &str, value: &str, maximum: usize) -> Result<String, String> {
    if value.trim().is_empty() {
        return Err(format!("{field} must be non-empty"));
    }
    if value.len() > maximum {
        return Err(format!("{field} exceeds the {maximum}-byte bound"));
    }
    if value.chars().any(|character| character.is_control()) {
        return Err(format!("{field} must not contain control characters"));
    }
    Ok(value.to_string())
}

fn digest(field: &str, value: &str) -> Result<String, String> {
    ContentHash::parse(value.to_string())
        .map(|digest| digest.to_string())
        .map_err(|_| format!("{field} must be a lowercase 64-character SHA-256 digest"))
}

fn optional_digest(field: &str, value: &Option<String>) -> Result<Option<String>, String> {
    value
        .as_deref()
        .map(|value| digest(field, value))
        .transpose()
}

fn validate_losses(losses: &[AdapterExecutionLoss]) -> Result<(), String> {
    if losses.len() > MAX_ADAPTER_EXECUTION_EVIDENCE_LOSSES {
        return Err(format!(
            "losses must contain at most {} entries",
            MAX_ADAPTER_EXECUTION_EVIDENCE_LOSSES
        ));
    }
    let mut identities = BTreeSet::new();
    for loss in losses {
        let kind = text("loss.kind", &loss.kind, 128)?;
        let severity = text("loss.severity", &loss.severity, 32)?;
        if !LOSS_SEVERITIES.contains(&severity.as_str()) {
            return Err(format!(
                "loss.severity must be one of {}",
                LOSS_SEVERITIES.join(", ")
            ));
        }
        let detail = text("loss.detail", &loss.detail, 512)?;
        let source_path = loss
            .source_path
            .as_deref()
            .map(|value| text("loss.source_path", value, 512))
            .transpose()?;
        let target_path = loss
            .target_path
            .as_deref()
            .map(|value| text("loss.target_path", value, 512))
            .transpose()?;
        let identity = json!({
            "kind": kind,
            "severity": severity,
            "detail": detail,
            "source_path": source_path,
            "target_path": target_path,
        });
        if !identities.insert(identity.to_string()) {
            return Err("loss entries must be unique".into());
        }
    }
    Ok(())
}

fn validate_request(request: &AdapterExecutionEvidenceRequest) -> Result<(), String> {
    text(
        "group_id",
        &request.group_id,
        MAX_ADAPTER_EXECUTION_EVIDENCE_TEXT_BYTES,
    )?;
    if request.domains.is_empty() || request.domains.len() > MAX_ADAPTER_EXECUTION_EVIDENCE_DOMAINS
    {
        return Err(format!(
            "domains must contain 1..={} entries",
            MAX_ADAPTER_EXECUTION_EVIDENCE_DOMAINS
        ));
    }
    for domain in &request.domains {
        text("domain", domain, MAX_ADAPTER_EXECUTION_EVIDENCE_TEXT_BYTES)?;
    }
    text(
        "subject_id",
        &request.subject_id,
        MAX_ADAPTER_EXECUTION_EVIDENCE_TEXT_BYTES,
    )?;
    text("adapter_id", &request.adapter_id, 256)?;
    text("adapter_version", &request.adapter_version, 128)?;
    text(
        "source_id",
        &request.source_id,
        MAX_ADAPTER_EXECUTION_EVIDENCE_TEXT_BYTES,
    )?;
    digest("input_digest", &request.input_digest)?;
    let output_digest = optional_digest("output_digest", &request.output_digest)?;
    text("execution_status", &request.execution_status, 32)?;
    if !EXECUTION_STATUSES.contains(&request.execution_status.as_str()) {
        return Err(format!(
            "execution_status must be one of {}",
            EXECUTION_STATUSES.join(", ")
        ));
    }
    text("conformance_status", &request.conformance_status, 32)?;
    if !CONFORMANCE_STATUSES.contains(&request.conformance_status.as_str()) {
        return Err(format!(
            "conformance_status must be one of {}",
            CONFORMANCE_STATUSES.join(", ")
        ));
    }
    text("semantic_loss_status", &request.semantic_loss_status, 32)?;
    if !SEMANTIC_LOSS_STATUSES.contains(&request.semantic_loss_status.as_str()) {
        return Err(format!(
            "semantic_loss_status must be one of {}",
            SEMANTIC_LOSS_STATUSES.join(", ")
        ));
    }
    validate_losses(&request.losses)?;
    if request.semantic_loss_status == "lossless" && !request.losses.is_empty() {
        return Err("lossless evidence cannot contain loss entries".into());
    }
    if request.semantic_loss_status == "lossy" && request.losses.is_empty() {
        return Err("lossy evidence must contain at least one loss entry".into());
    }
    if request.semantic_loss_status == "not_applicable" && !request.losses.is_empty() {
        return Err("not_applicable semantic loss cannot contain loss entries".into());
    }
    if request.execution_status == "succeeded" && output_digest.is_none() {
        return Err("succeeded execution requires output_digest".into());
    }
    if matches!(request.execution_status.as_str(), "refused" | "failed")
        && request.error_code.is_none()
    {
        return Err("refused or failed execution requires error_code".into());
    }
    if let Some(error_code) = &request.error_code {
        text("error_code", error_code, 128)?;
    }
    if request
        .item_count
        .is_some_and(|value| value > MAX_ADAPTER_EXECUTION_EVIDENCE_ITEMS)
    {
        return Err(format!(
            "item_count exceeds {}",
            MAX_ADAPTER_EXECUTION_EVIDENCE_ITEMS
        ));
    }
    if request
        .byte_length
        .is_some_and(|value| value > MAX_ADAPTER_EXECUTION_EVIDENCE_BYTES)
    {
        return Err(format!(
            "byte_length exceeds {}",
            MAX_ADAPTER_EXECUTION_EVIDENCE_BYTES
        ));
    }
    if request.parent_digests.len() > MAX_ADAPTER_EXECUTION_EVIDENCE_PARENTS {
        return Err(format!(
            "parent_digests must contain at most {} entries",
            MAX_ADAPTER_EXECUTION_EVIDENCE_PARENTS
        ));
    }
    for parent in &request.parent_digests {
        digest("parent_digest", parent)?;
    }
    if let Some(attempt_id) = &request.attempt_id {
        text("attempt_id", attempt_id, 128)?;
    }
    Ok(())
}

/// Canonicalize caller-supplied adapter observations into a digest-bound evidence envelope.
pub fn record_adapter_execution_evidence(
    request: AdapterExecutionEvidenceRequest,
) -> Result<Value, String> {
    validate_request(&request)?;
    let output_digest = optional_digest("output_digest", &request.output_digest)?;
    let evidence_without_digest = json!({
        "schema": ADAPTER_EXECUTION_EVIDENCE_SCHEMA,
        "workflow": ADAPTER_EXECUTION_EVIDENCE_WORKFLOW,
        "group_id": request.group_id,
        "domains": request.domains,
        "subject_id": request.subject_id,
        "adapter_id": request.adapter_id,
        "adapter_version": request.adapter_version,
        "source_id": request.source_id,
        "input_digest": request.input_digest,
        "output_digest": output_digest,
        "execution_status": request.execution_status,
        "conformance_status": request.conformance_status,
        "semantic_loss_status": request.semantic_loss_status,
        "losses": request.losses,
        "item_count": request.item_count,
        "byte_length": request.byte_length,
        "error_code": request.error_code,
        "parent_digests": request.parent_digests,
        "attempt_id": request.attempt_id,
        "attestation_posture": "caller_asserted",
    });
    let evidence_digest = ContentHash::of_value(&evidence_without_digest)
        .map_err(|error| format!("cannot digest adapter execution evidence: {error}"))?
        .to_string();
    let mut evidence = evidence_without_digest;
    evidence["evidence_digest"] = json!(evidence_digest);
    let result = json!({
        "ok": true,
        "schema": ADAPTER_EXECUTION_EVIDENCE_SCHEMA,
        "workflow": ADAPTER_EXECUTION_EVIDENCE_WORKFLOW,
        "evidence": evidence,
        "evidence_digest": evidence_digest,
        "attestation_posture": "caller_asserted",
        "execution": "not_started",
        "readiness_claimed": false,
        "guarantees": [
            "the selected adapter, source, input identity, and caller outcome are retained together",
            "conformance and semantic-loss status remain independent and explicit",
            "the evidence digest covers every supplied metadata field except its own digest"
        ],
        "does_not_claim": [
            "the MCP core executed an adapter or imported an optional dependency",
            "caller_asserted execution_status or conformance_status proves adapter correctness",
            "lossless or verified posture proves scientific, clinical, provenance, regulatory, release, or readiness validity"
        ]
    });
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request() -> AdapterExecutionEvidenceRequest {
        AdapterExecutionEvidenceRequest {
            group_id: "biological_domains".into(),
            domains: vec!["genomics".into()],
            subject_id: "subject-1".into(),
            adapter_id: "bioprism.python.vcf_text".into(),
            adapter_version: "0.1.0".into(),
            source_id: "vcf-1".into(),
            input_digest: "a".repeat(64),
            output_digest: Some("b".repeat(64)),
            execution_status: "succeeded".into(),
            conformance_status: "verified".into(),
            semantic_loss_status: "lossless".into(),
            losses: vec![],
            item_count: Some(4),
            byte_length: Some(128),
            error_code: None,
            parent_digests: vec!["c".repeat(64)],
            attempt_id: Some("attempt-1".into()),
        }
    }

    #[test]
    fn evidence_is_digest_bound_and_non_executing() {
        let result = record_adapter_execution_evidence(request()).unwrap();
        assert!(result["ok"].as_bool().unwrap());
        assert_eq!(result["execution"], "not_started");
        assert!(!result["readiness_claimed"].as_bool().unwrap());
        assert_eq!(
            result["evidence"]["evidence_digest"],
            result["evidence_digest"]
        );
    }

    #[test]
    fn loss_and_refusal_states_require_explicit_evidence() {
        let mut request = request();
        request.semantic_loss_status = "lossy".into();
        assert!(record_adapter_execution_evidence(request.clone()).is_err());
        request.semantic_loss_status = "unknown".into();
        request.execution_status = "refused".into();
        assert!(record_adapter_execution_evidence(request).is_err());
    }
}
