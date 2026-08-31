//! Joined, read-only projections for external payload evidence.
//!
//! The artifact registry stores receipts, lineage audits, and caller execution evidence as
//! independent digest-addressed records. This module joins those records by the receipt digest
//! for operator queries while preserving missing joins, cursor bounds, and non-claiming posture.

use crate::artifact_registry::ArtifactRecord;
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};

pub const DOMAIN_EVIDENCE_PROVIDER_EXTERNAL_PAYLOAD_QUERY_SCHEMA: &str =
    "bioprism-devplat-domain-evidence-provider-external-payload-query/0.1";
pub const DOMAIN_EVIDENCE_PROVIDER_EXTERNAL_PAYLOAD_QUERY_WORKFLOW: &str =
    "domain_evidence_provider_external_payload_evidence_query";
pub const MAX_DOMAIN_EVIDENCE_PROVIDER_EXTERNAL_PAYLOAD_QUERY_ITEMS: usize = 128;
const MAX_TEXT_BYTES: usize = 512;
const MAX_ARRAY_VALUES: usize = 256;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
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

impl Default for DomainEvidenceProviderExternalPayloadEvidenceQueryRequest {
    fn default() -> Self {
        Self {
            group_id: None,
            domain: None,
            subject_id: None,
            after: None,
            max_items: default_max_items(),
            include_artifacts: false,
        }
    }
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
    lineage_seen: bool,
    execution_seen: bool,
}

fn valid_text(value: &str) -> bool {
    let trimmed = value.trim();
    !trimmed.is_empty()
        && value == trimmed
        && value.len() <= MAX_TEXT_BYTES
        && !value.chars().any(char::is_control)
}

fn valid_digest(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        && ContentHash::parse(value.to_owned()).is_ok()
}

fn text(value: Option<&Value>, field: &str) -> Result<Option<String>, String> {
    match value {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(value)) if valid_text(value) => Ok(Some(value.clone())),
        Some(Value::String(_)) => Err(format!("{field} must be bounded visible text")),
        Some(_) => Err(format!("{field} must be text or null")),
    }
}

fn required_text(value: Option<&Value>, field: &str) -> Result<String, String> {
    text(value, field)?.ok_or_else(|| format!("{field} is required for a joined evidence row"))
}

fn digest(value: Option<&Value>, field: &str) -> Result<Option<String>, String> {
    match value {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(value)) if valid_digest(value) => Ok(Some(value.clone())),
        Some(Value::String(_)) => Err(format!("{field} must be a lowercase SHA-256 digest")),
        Some(_) => Err(format!("{field} must be a digest or null")),
    }
}

fn array_text(value: Option<&Value>, field: &str) -> Result<Vec<String>, String> {
    let Some(value) = value else {
        return Ok(Vec::new());
    };
    let items = value
        .as_array()
        .ok_or_else(|| format!("{field} must be an array"))?;
    if items.len() > MAX_ARRAY_VALUES {
        return Err(format!(
            "{field} must contain at most {MAX_ARRAY_VALUES} entries"
        ));
    }
    let domain_values = field.ends_with("domains");
    let digest_values = field.ends_with("parent_digests");
    let mut seen = BTreeSet::new();
    let values = items
        .iter()
        .enumerate()
        .map(|(index, item)| {
            let value = text(Some(item), &format!("{field}[{index}]"))?
                .ok_or_else(|| format!("{field}[{index}] must not be null"))?;
            if digest_values && !valid_digest(&value) {
                return Err(format!(
                    "{field}[{index}] must be a lowercase SHA-256 digest"
                ));
            }
            let identity = if domain_values {
                value.to_ascii_lowercase()
            } else {
                value.clone()
            };
            if !seen.insert(identity) {
                return Err(format!(
                    "{field} must not contain duplicate or case-colliding values"
                ));
            }
            Ok(value)
        })
        .collect::<Result<Vec<_>, _>>()?;
    let mut canonical = values.clone();
    canonical.sort();
    if values != canonical {
        return Err(format!("{field} must use canonical sorted order"));
    }
    Ok(values)
}

fn required_array_text(value: Option<&Value>, field: &str) -> Result<Vec<String>, String> {
    let values = array_text(value, field)?;
    if values.is_empty() {
        return Err(format!("{field} must contain at least one value"));
    }
    Ok(values)
}

fn merge_text(slot: &mut String, value: String, field: &str) -> Result<(), String> {
    if slot.is_empty() {
        *slot = value;
        return Ok(());
    }
    if *slot != value {
        return Err(format!(
            "joined {field} values conflict for one receipt digest"
        ));
    }
    Ok(())
}

fn merge_texts(slot: &mut Vec<String>, values: Vec<String>, field: &str) -> Result<(), String> {
    if slot.is_empty() {
        *slot = values;
        return Ok(());
    }
    if *slot != values {
        return Err(format!(
            "joined {field} values conflict for one receipt digest"
        ));
    }
    Ok(())
}

fn merge_artifact(slot: &mut Option<Value>, value: Value, field: &str) -> Result<(), String> {
    if slot.as_ref().is_some_and(|existing| existing != &value) {
        return Err(format!(
            "joined {field} artifacts conflict for one receipt digest"
        ));
    }
    if slot.is_none() {
        *slot = Some(value);
    }
    Ok(())
}

fn row_digest(row: &DomainEvidenceProviderExternalPayloadEvidenceRow) -> Result<String, String> {
    let mut value = serde_json::to_value(row).map_err(|error| error.to_string())?;
    let Some(object) = value.as_object_mut() else {
        return Err("evidence row did not serialize as an object".into());
    };
    object.remove("row_digest");
    ContentHash::of_value(&value)
        .map(|digest| digest.to_string())
        .map_err(|error| error.to_string())
}

fn add_record(
    rows: &mut BTreeMap<String, PartialRow>,
    record: &ArtifactRecord,
    include_artifacts: bool,
) -> Result<(), String> {
    let artifact = &record.artifact;
    if !valid_digest(&record.content_digest) {
        return Err("record.content_digest must be a lowercase SHA-256 digest".into());
    }
    let (receipt_digest, receipt, kind) = match record.kind.as_str() {
        "domain_evidence_provider_external_payload" => (
            digest(artifact.get("receipt_digest"), "receipt_digest")?,
            artifact,
            "receipt",
        ),
        "domain_evidence_provider_external_payload_lineage_audit" => {
            let receipt = artifact.get("receipt").and_then(Value::as_object);
            (
                receipt
                    .map(|value| digest(value.get("receipt_digest"), "receipt.receipt_digest"))
                    .transpose()?
                    .flatten(),
                artifact,
                "lineage",
            )
        }
        "domain_evidence_provider_external_payload_execution_evidence" => {
            let receipt = artifact.get("receipt").and_then(Value::as_object);
            (
                receipt
                    .map(|value| digest(value.get("receipt_digest"), "receipt.receipt_digest"))
                    .transpose()?
                    .flatten(),
                artifact,
                "execution",
            )
        }
        _ => return Ok(()),
    };
    let Some(receipt_digest) = receipt_digest else {
        return Ok(());
    };
    let entry = rows
        .entry(receipt_digest.clone())
        .or_insert_with(|| PartialRow {
            receipt_digest,
            ..PartialRow::default()
        });
    if kind == "receipt" {
        entry.receipt_present = true;
        merge_text(
            &mut entry.subject_id,
            required_text(artifact.get("subject_id"), "subject_id")?,
            "subject_id",
        )?;
        merge_text(
            &mut entry.group_id,
            required_text(artifact.get("group_id"), "group_id")?,
            "group_id",
        )?;
        merge_texts(
            &mut entry.domains,
            required_array_text(artifact.get("domains"), "domains")?,
            "domains",
        )?;
        merge_texts(
            &mut entry.parent_digests,
            array_text(artifact.get("parent_digests"), "parent_digests")?,
            "parent_digests",
        )?;
        if include_artifacts {
            merge_artifact(&mut entry.receipt_artifact, receipt.clone(), "receipt")?;
        }
    } else {
        let nested_receipt = artifact
            .get("receipt")
            .and_then(Value::as_object)
            .ok_or_else(|| {
                "joined lineage or execution evidence requires a receipt object".to_string()
            })?;
        merge_text(
            &mut entry.subject_id,
            required_text(nested_receipt.get("subject_id"), "receipt.subject_id")?,
            "subject_id",
        )?;
        merge_text(
            &mut entry.group_id,
            required_text(nested_receipt.get("group_id"), "receipt.group_id")?,
            "group_id",
        )?;
        merge_texts(
            &mut entry.domains,
            required_array_text(nested_receipt.get("domains"), "receipt.domains")?,
            "domains",
        )?;
        if kind == "lineage" {
            let lineage_status = text(artifact.get("lineage_status"), "lineage_status")?;
            let lineage_digest = digest(artifact.get("lineage_digest"), "lineage_digest")?;
            if entry.lineage_seen
                && (entry.lineage_status != lineage_status
                    || entry.lineage_digest != lineage_digest)
            {
                return Err("joined lineage evidence conflicts for one receipt digest".into());
            }
            entry.lineage_seen = true;
            entry.lineage_status = lineage_status;
            entry.lineage_digest = lineage_digest;
            if include_artifacts {
                merge_artifact(&mut entry.lineage_artifact, artifact.clone(), "lineage")?;
            }
        } else {
            let execution_evidence_status =
                text(artifact.get("evidence_status"), "evidence_status")?;
            let execution_status = text(artifact.get("execution_status"), "execution_status")?;
            let evidence_digest = digest(artifact.get("evidence_digest"), "evidence_digest")?;
            if entry.execution_seen
                && (entry.execution_evidence_status != execution_evidence_status
                    || entry.execution_status != execution_status
                    || entry.evidence_digest != evidence_digest)
            {
                return Err("joined execution evidence conflicts for one receipt digest".into());
            }
            entry.execution_seen = true;
            entry.execution_evidence_status = execution_evidence_status;
            entry.execution_status = execution_status;
            entry.evidence_digest = evidence_digest;
            if include_artifacts {
                merge_artifact(&mut entry.execution_artifact, artifact.clone(), "execution")?;
            }
        }
    }
    Ok(())
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
        if value.is_some_and(|value| !valid_text(value) || value != value.trim()) {
            return Err(format!(
                "{name} must be bounded visible text without surrounding whitespace"
            ));
        }
    }
    if let Some(after) = request.after.as_deref() {
        if !valid_digest(after) {
            return Err("after must be a lowercase SHA-256 digest".into());
        }
    }
    let mut partial_rows = BTreeMap::new();
    for record in records {
        add_record(&mut partial_rows, record, request.include_artifacts)?;
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

    #[test]
    fn query_rejects_invalid_nested_identity_and_digest_values() {
        let records = vec![record(
            "domain_evidence_provider_external_payload",
            json!({
                "receipt_digest": "A".repeat(64),
                "subject_id": "subject-1",
                "group_id": "biological_domains",
                "domains": ["oncology"],
                "parent_digests": []
            }),
        )];
        let error = query_domain_evidence_provider_external_payload_evidence(
            &records,
            1,
            DomainEvidenceProviderExternalPayloadEvidenceQueryRequest {
                max_items: 1,
                ..Default::default()
            },
        )
        .expect_err("uppercase receipt digests must be rejected");
        assert!(error.contains("receipt_digest"));

        let records = vec![record(
            "domain_evidence_provider_external_payload",
            json!({
                "receipt_digest": "b".repeat(64),
                "subject_id": "subject\n1",
                "group_id": "biological_domains",
                "domains": ["oncology"],
                "parent_digests": []
            }),
        )];
        let error = query_domain_evidence_provider_external_payload_evidence(
            &records,
            1,
            DomainEvidenceProviderExternalPayloadEvidenceQueryRequest {
                max_items: 1,
                ..Default::default()
            },
        )
        .expect_err("control-bearing identity must not be projected");
        assert!(error.contains("subject_id"));
    }

    #[test]
    fn query_rejects_noncanonical_join_identity_arrays() {
        let receipt_digest = "b".repeat(64);
        let mut artifact = json!({
            "receipt_digest": receipt_digest,
            "subject_id": "subject-1",
            "group_id": "biological_domains",
            "domains": ["oncology", "genomics"],
            "parent_digests": []
        });
        let error = query_domain_evidence_provider_external_payload_evidence(
            &[record(
                "domain_evidence_provider_external_payload",
                artifact.clone(),
            )],
            1,
            DomainEvidenceProviderExternalPayloadEvidenceQueryRequest::default(),
        )
        .expect_err("joined domains must use canonical order");
        assert!(error.contains("domains"));

        artifact["domains"] = json!(["oncology"]);
        artifact["parent_digests"] = json!(["d".repeat(64), "c".repeat(64)]);
        let error = query_domain_evidence_provider_external_payload_evidence(
            &[record(
                "domain_evidence_provider_external_payload",
                artifact.clone(),
            )],
            1,
            DomainEvidenceProviderExternalPayloadEvidenceQueryRequest::default(),
        )
        .expect_err("joined parents must use canonical order");
        assert!(error.contains("parent_digests"));

        artifact["parent_digests"] = json!([]);
        artifact["domains"] = json!(["oncology", "oncology"]);
        let error = query_domain_evidence_provider_external_payload_evidence(
            &[record(
                "domain_evidence_provider_external_payload",
                artifact,
            )],
            1,
            DomainEvidenceProviderExternalPayloadEvidenceQueryRequest::default(),
        )
        .expect_err("joined domain duplicates must not be silently collapsed");
        assert!(error.contains("domains"));
    }

    #[test]
    fn query_rejects_missing_and_conflicting_join_identity() {
        let receipt_digest = "b".repeat(64);
        let missing_subject = vec![record(
            "domain_evidence_provider_external_payload",
            json!({
                "receipt_digest": receipt_digest.clone(),
                "group_id": "biological_domains",
                "domains": ["oncology"],
                "parent_digests": []
            }),
        )];
        let error = query_domain_evidence_provider_external_payload_evidence(
            &missing_subject,
            1,
            DomainEvidenceProviderExternalPayloadEvidenceQueryRequest {
                max_items: 1,
                ..Default::default()
            },
        )
        .expect_err("receipt identity must not default to an empty subject");
        assert!(error.contains("subject_id"));

        let conflicting = vec![
            record(
                "domain_evidence_provider_external_payload",
                json!({
                    "receipt_digest": receipt_digest.clone(),
                    "subject_id": "subject-1",
                    "group_id": "biological_domains",
                    "domains": ["oncology"],
                    "parent_digests": []
                }),
            ),
            record(
                "domain_evidence_provider_external_payload_lineage_audit",
                json!({
                    "receipt": {
                        "receipt_digest": receipt_digest,
                        "subject_id": "subject-2",
                        "group_id": "biological_domains",
                        "domains": ["oncology"]
                    },
                    "lineage_status": "matched",
                    "lineage_digest": "c".repeat(64)
                }),
            ),
        ];
        let error = query_domain_evidence_provider_external_payload_evidence(
            &conflicting,
            1,
            DomainEvidenceProviderExternalPayloadEvidenceQueryRequest {
                max_items: 1,
                ..Default::default()
            },
        )
        .expect_err("conflicting joined identities must be refused");
        assert!(error.contains("subject_id"));
    }

    #[test]
    fn query_rejects_conflicting_duplicate_join_records_instead_of_overwriting() {
        let receipt_digest = "b".repeat(64);
        let records = vec![
            record(
                "domain_evidence_provider_external_payload_lineage_audit",
                json!({
                    "receipt": {
                        "receipt_digest": receipt_digest.clone(),
                        "subject_id": "subject-1",
                        "group_id": "biological_domains",
                        "domains": ["oncology"]
                    },
                    "lineage_status": "matched",
                    "lineage_digest": "c".repeat(64),
                    "operator_note": "first"
                }),
            ),
            record(
                "domain_evidence_provider_external_payload_lineage_audit",
                json!({
                    "receipt": {
                        "receipt_digest": receipt_digest.clone(),
                        "subject_id": "subject-1",
                        "group_id": "biological_domains",
                        "domains": ["oncology"]
                    },
                    "lineage_status": "matched",
                    "lineage_digest": "c".repeat(64),
                    "operator_note": "second"
                }),
            ),
        ];
        let error = query_domain_evidence_provider_external_payload_evidence(
            &records,
            1,
            DomainEvidenceProviderExternalPayloadEvidenceQueryRequest {
                include_artifacts: true,
                ..Default::default()
            },
        )
        .expect_err("conflicting retained lineage artifacts must not be overwritten");
        assert!(error.contains("lineage artifacts"));

        let records = vec![
            record(
                "domain_evidence_provider_external_payload_execution_evidence",
                json!({
                    "receipt": {
                        "receipt_digest": receipt_digest.clone(),
                        "subject_id": "subject-1",
                        "group_id": "biological_domains",
                        "domains": ["oncology"]
                    },
                    "evidence_status": "partial",
                    "execution_status": "transferred",
                    "evidence_digest": "d".repeat(64)
                }),
            ),
            record(
                "domain_evidence_provider_external_payload_execution_evidence",
                json!({
                    "receipt": {
                        "receipt_digest": receipt_digest,
                        "subject_id": "subject-1",
                        "group_id": "biological_domains",
                        "domains": ["oncology"]
                    },
                    "evidence_status": "mismatch",
                    "execution_status": "transferred",
                    "evidence_digest": "d".repeat(64)
                }),
            ),
        ];
        let error = query_domain_evidence_provider_external_payload_evidence(
            &records,
            1,
            DomainEvidenceProviderExternalPayloadEvidenceQueryRequest::default(),
        )
        .expect_err("conflicting execution projections must not be overwritten");
        assert!(error.contains("execution evidence"));
    }
}
