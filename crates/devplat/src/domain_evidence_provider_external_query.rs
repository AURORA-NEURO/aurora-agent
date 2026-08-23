//! Joined, read-only projections for external payload evidence.
//!
//! The artifact registry stores receipts, lineage audits, and caller execution evidence as
//! independent digest-addressed records. This module joins those records by the receipt digest
//! for operator queries while preserving missing joins, cursor bounds, and non-claiming posture.

use crate::artifact_registry::ArtifactRecord;
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::BTreeMap;

pub const DOMAIN_EVIDENCE_PROVIDER_EXTERNAL_PAYLOAD_QUERY_SCHEMA: &str =
    "bioprism-devplat-domain-evidence-provider-external-payload-query/0.1";
pub const DOMAIN_EVIDENCE_PROVIDER_EXTERNAL_PAYLOAD_QUERY_WORKFLOW: &str =
    "domain_evidence_provider_external_payload_evidence_query";
pub const MAX_DOMAIN_EVIDENCE_PROVIDER_EXTERNAL_PAYLOAD_QUERY_ITEMS: usize = 128;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(deny_unknown_fields)]
pub struct DomainEvidenceProviderExternalPayloadEvidenceQueryRequest {
    #[serde(default)]
    pub group_id: Option<String>,
    #[serde(default)]
    pub domain: Option<String>,
    #[serde(default)]
    pub subject_id: Option<String>,
    #[serde(default)]
    pub after: Option<String>,
    #[serde(default = "default_max_items")]
    pub max_items: usize,
    #[serde(default)]
    pub include_artifacts: bool,
}

fn default_max_items() -> usize {
    100
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DomainEvidenceProviderExternalPayloadEvidenceRow {
    pub row_digest: String,
    pub receipt_digest: String,
    pub subject_id: String,
    pub group_id: String,
    pub domains: Vec<String>,
    pub receipt_present: bool,
    pub lineage_status: Option<String>,
    pub lineage_digest: Option<String>,
    pub execution_evidence_status: Option<String>,
    pub execution_status: Option<String>,
    pub evidence_digest: Option<String>,
    pub join_status: String,
    pub parent_digests: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub receipt_artifact: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lineage_artifact: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub execution_artifact: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DomainEvidenceProviderExternalPayloadEvidenceQueryReport {
    pub ok: bool,
    pub schema: String,
    pub workflow: String,
    pub filters: DomainEvidenceProviderExternalPayloadEvidenceQueryRequest,
    pub registry_generation: u64,
    pub registry_size: usize,
    pub rows: Vec<DomainEvidenceProviderExternalPayloadEvidenceRow>,
    pub next_after: Option<String>,
    pub has_more: bool,
    pub query_digest: String,
    pub execution: String,
    pub readiness_claimed: bool,
    pub guarantees: Vec<String>,
    pub limitations: Vec<String>,
}

#[derive(Default)]
struct PartialRow {
    receipt_digest: String,
    subject_id: String,
    group_id: String,
    domains: Vec<String>,
    receipt_present: bool,
    lineage_status: Option<String>,
    lineage_digest: Option<String>,
    execution_evidence_status: Option<String>,
    execution_status: Option<String>,
    evidence_digest: Option<String>,
    parent_digests: Vec<String>,
    receipt_artifact: Option<Value>,
    lineage_artifact: Option<Value>,
    execution_artifact: Option<Value>,
}

fn text(value: Option<&Value>) -> Option<String> {
    value.and_then(Value::as_str).map(ToOwned::to_owned)
}

fn digest(value: Option<&Value>) -> Option<String> {
    text(value).filter(|value| ContentHash::parse(value.to_owned()).is_ok())
}

fn array_text(value: Option<&Value>) -> Vec<String> {
    value
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|item| item.as_str().map(ToOwned::to_owned))
                .collect()
        })
        .unwrap_or_default()
}

fn row_digest(row: &DomainEvidenceProviderExternalPayloadEvidenceRow) -> Result<String, String> {
    let mut value = serde_json::to_value(row).map_err(|error| error.to_string())?;
    value
        .as_object_mut()
        .expect("evidence row serializes as an object")
        .remove("row_digest");
    ContentHash::of_value(&value)
        .map(|digest| digest.to_string())
        .map_err(|error| error.to_string())
}

fn add_record(
    rows: &mut BTreeMap<String, PartialRow>,
    record: &ArtifactRecord,
    include_artifacts: bool,
) {
    let artifact = &record.artifact;
    let (receipt_digest, receipt, kind) = match record.kind.as_str() {
        "domain_evidence_provider_external_payload" => {
            (digest(artifact.get("receipt_digest")), artifact, "receipt")
        }
        "domain_evidence_provider_external_payload_lineage_audit" => {
            let receipt = artifact.get("receipt").and_then(Value::as_object);
            (
                receipt.and_then(|value| digest(value.get("receipt_digest"))),
                artifact,
                "lineage",
            )
        }
        "domain_evidence_provider_external_payload_execution_evidence" => {
            let receipt = artifact.get("receipt").and_then(Value::as_object);
            (
                receipt.and_then(|value| digest(value.get("receipt_digest"))),
                artifact,
                "execution",
            )
        }
        _ => return,
    };
    let Some(receipt_digest) = receipt_digest else {
        return;
    };
    let entry = rows
        .entry(receipt_digest.clone())
        .or_insert_with(|| PartialRow {
            receipt_digest,
            ..PartialRow::default()
        });
    if kind == "receipt" {
        entry.receipt_present = true;
        entry.subject_id = text(artifact.get("subject_id")).unwrap_or_default();
        entry.group_id = text(artifact.get("group_id")).unwrap_or_default();
        entry.domains = array_text(artifact.get("domains"));
        entry.parent_digests = array_text(artifact.get("parent_digests"));
        if include_artifacts {
            entry.receipt_artifact = Some(receipt.clone());
        }
    } else {
        let nested_receipt = artifact.get("receipt").and_then(Value::as_object);
        if entry.subject_id.is_empty() {
            entry.subject_id = nested_receipt
                .and_then(|value| text(value.get("subject_id")))
                .unwrap_or_default();
        }
        if entry.group_id.is_empty() {
            entry.group_id = nested_receipt
                .and_then(|value| text(value.get("group_id")))
                .unwrap_or_default();
        }
        if entry.domains.is_empty() {
            entry.domains = nested_receipt
                .map(|value| array_text(value.get("domains")))
                .unwrap_or_default();
        }
        if kind == "lineage" {
            entry.lineage_status = text(artifact.get("lineage_status"));
            entry.lineage_digest = digest(artifact.get("lineage_digest"));
            if include_artifacts {
                entry.lineage_artifact = Some(artifact.clone());
            }
        } else {
            entry.execution_evidence_status = text(artifact.get("evidence_status"));
            entry.execution_status = artifact
                .get("execution_status")
                .and_then(Value::as_str)
                .map(ToOwned::to_owned);
            entry.evidence_digest = digest(artifact.get("evidence_digest"));
            if include_artifacts {
                entry.execution_artifact = Some(artifact.clone());
            }
        }
    }
}

/// Build a deterministic joined projection from validated registry records.
pub fn query_domain_evidence_provider_external_payload_evidence(
    records: &[ArtifactRecord],
    generation: u64,
    request: DomainEvidenceProviderExternalPayloadEvidenceQueryRequest,
) -> Result<DomainEvidenceProviderExternalPayloadEvidenceQueryReport, String> {
    if request.max_items == 0
        || request.max_items > MAX_DOMAIN_EVIDENCE_PROVIDER_EXTERNAL_PAYLOAD_QUERY_ITEMS
    {
        return Err(format!(
            "max_items must be between 1 and {}",
            MAX_DOMAIN_EVIDENCE_PROVIDER_EXTERNAL_PAYLOAD_QUERY_ITEMS
        ));
    }
    for (name, value) in [
        ("group_id", request.group_id.as_ref()),
        ("domain", request.domain.as_ref()),
        ("subject_id", request.subject_id.as_ref()),
    ] {
        if value.is_some_and(|value| value.trim().is_empty() || value.len() > 512) {
            return Err(format!(
                "{name} must be non-empty text of at most 512 bytes"
            ));
        }
    }
    if let Some(after) = request.after.as_deref() {
        digest(Some(&Value::String(after.to_owned()))).ok_or("after must be a digest")?;
    }
    let mut partial_rows = BTreeMap::new();
    for record in records {
        add_record(&mut partial_rows, record, request.include_artifacts);
    }
    let mut rows = Vec::new();
    for (_, partial) in partial_rows {
        if request
            .after
            .as_deref()
            .is_some_and(|after| partial.receipt_digest.as_str() <= after)
        {
            continue;
        }
        if request
            .group_id
            .as_deref()
            .is_some_and(|group| partial.group_id != group)
            || request
                .subject_id
                .as_deref()
                .is_some_and(|subject| partial.subject_id != subject)
            || request
                .domain
                .as_deref()
                .is_some_and(|domain| !partial.domains.iter().any(|item| item == domain))
        {
            continue;
        }
        let join_status = match (
            partial.receipt_present,
            partial.lineage_status.is_some(),
            partial.execution_evidence_status.is_some(),
        ) {
            (false, _, _) => "missing_receipt",
            (true, true, true) => "complete",
            (true, true, false) => "receipt_and_lineage",
            (true, false, true) => "receipt_and_execution",
            (true, false, false) => "receipt_only",
        };
        let mut row = DomainEvidenceProviderExternalPayloadEvidenceRow {
            row_digest: String::new(),
            receipt_digest: partial.receipt_digest,
            subject_id: partial.subject_id,
            group_id: partial.group_id,
            domains: partial.domains,
            receipt_present: partial.receipt_present,
            lineage_status: partial.lineage_status,
            lineage_digest: partial.lineage_digest,
            execution_evidence_status: partial.execution_evidence_status,
            execution_status: partial.execution_status,
            evidence_digest: partial.evidence_digest,
            join_status: join_status.into(),
            parent_digests: partial.parent_digests,
            receipt_artifact: partial.receipt_artifact,
            lineage_artifact: partial.lineage_artifact,
            execution_artifact: partial.execution_artifact,
        };
        row.row_digest = row_digest(&row)?;
        rows.push(row);
        if rows.len() == request.max_items + 1 {
            break;
        }
    }
    let has_more = rows.len() > request.max_items;
    if has_more {
        rows.pop();
    }
    let next_after = if has_more {
        rows.last().map(|row| row.receipt_digest.clone())
    } else {
        None
    };
    let filters = serde_json::to_value(&request).map_err(|error| error.to_string())?;
    let query_digest = ContentHash::of_value(&json!({
        "schema": DOMAIN_EVIDENCE_PROVIDER_EXTERNAL_PAYLOAD_QUERY_SCHEMA,
        "workflow": DOMAIN_EVIDENCE_PROVIDER_EXTERNAL_PAYLOAD_QUERY_WORKFLOW,
        "filters": filters,
        "rows": rows
    }))
    .map(|digest| digest.to_string())
    .map_err(|error| error.to_string())?;
    Ok(DomainEvidenceProviderExternalPayloadEvidenceQueryReport {
        ok: true,
        schema: DOMAIN_EVIDENCE_PROVIDER_EXTERNAL_PAYLOAD_QUERY_SCHEMA.into(),
        workflow: DOMAIN_EVIDENCE_PROVIDER_EXTERNAL_PAYLOAD_QUERY_WORKFLOW.into(),
        filters: request,
        registry_generation: generation,
        registry_size: records.len(),
        rows,
        next_after,
        has_more,
        query_digest,
        execution: "not_started".into(),
        readiness_claimed: false,
        guarantees: vec![
            "receipt, lineage, and execution artifacts are joined only by explicit receipt digest".into(),
            "missing joins and partial projections remain visible in join_status".into(),
            "cursoring and row identity are deterministic over the bounded local registry".into(),
        ],
        limitations: vec![
            "the projection does not fetch providers, stores, locators, credentials, or payloads".into(),
            "joined registry evidence does not prove provider authenticity, transfer causality, or domain validity".into(),
            "readiness, execution authority, scientific, clinical, provenance, regulatory, and release validity remain unclaimed".into(),
        ],
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record(kind: &str, artifact: Value) -> ArtifactRecord {
        ArtifactRecord {
            content_digest: format!("{}{}", "a".repeat(63), kind.len() % 10),
            kind: kind.into(),
            subject_id: "subject-1".into(),
            domains: vec!["oncology".into()],
            parent_digests: vec![],
            declared_digest: None,
            verification: json!({"state": "verified_integrity"}),
            artifact,
        }
    }

    #[test]
    fn query_joins_partial_and_complete_external_payload_rows_with_cursoring() {
        let receipt_digest = "b".repeat(64);
        let records = vec![
            record(
                "domain_evidence_provider_external_payload",
                json!({"receipt_digest": receipt_digest, "subject_id": "subject-1", "group_id": "biological_domains", "domains": ["oncology"], "parent_digests": []}),
            ),
            record(
                "domain_evidence_provider_external_payload_lineage_audit",
                json!({"receipt": {"receipt_digest": receipt_digest, "subject_id": "subject-1", "group_id": "biological_domains", "domains": ["oncology"]}, "lineage_status": "matched", "lineage_digest": "c".repeat(64)}),
            ),
            record(
                "domain_evidence_provider_external_payload_execution_evidence",
                json!({"receipt": {"receipt_digest": receipt_digest, "subject_id": "subject-1", "group_id": "biological_domains", "domains": ["oncology"]}, "evidence_status": "partial", "execution_status": "transferred", "evidence_digest": "d".repeat(64)}),
            ),
        ];
        let report = query_domain_evidence_provider_external_payload_evidence(
            &records,
            4,
            DomainEvidenceProviderExternalPayloadEvidenceQueryRequest {
                max_items: 1,
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(report.rows.len(), 1);
        assert_eq!(report.rows[0].join_status, "complete");
        assert_eq!(
            report.rows[0].execution_evidence_status.as_deref(),
            Some("partial")
        );
        assert!(!report.has_more);
        assert!(!report.readiness_claimed);
    }
}
