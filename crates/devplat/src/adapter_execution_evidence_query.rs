//! Joined, read-only projections for retained adapter execution evidence.
//!
//! Adapter execution is intentionally delegated to native or Python-owned callers. Once a
//! caller submits an observation, this module lets operators find it again and inspect whether
//! it is explicitly linked to a retained source projection or workflow reconciliation. Parent
//! links are classified only when the registry contains the referenced digest; no semantic or
//! causal relationship is inferred from matching labels.

use crate::artifact_registry::ArtifactRecord;
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::BTreeMap;

pub const ADAPTER_EXECUTION_EVIDENCE_QUERY_SCHEMA: &str =
    "bioprism-devplat-adapter-execution-evidence-query/0.1";
pub const ADAPTER_EXECUTION_EVIDENCE_QUERY_WORKFLOW: &str = "adapter_execution_evidence_query";
pub const MAX_ADAPTER_EXECUTION_EVIDENCE_QUERY_ITEMS: usize = 128;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct AdapterExecutionEvidenceQueryRequest {
    #[serde(default)]
    pub group_id: Option<String>,
    #[serde(default)]
    pub domain: Option<String>,
    #[serde(default)]
    pub subject_id: Option<String>,
    #[serde(default)]
    pub adapter_id: Option<String>,
    #[serde(default)]
    pub source_id: Option<String>,
    #[serde(default)]
    pub execution_status: Option<String>,
    #[serde(default)]
    pub conformance_status: Option<String>,
    #[serde(default)]
    pub semantic_loss_status: Option<String>,
    #[serde(default)]
    pub after: Option<String>,
    #[serde(default = "default_max_items")]
    pub max_items: usize,
    #[serde(default)]
    pub include_artifacts: bool,
}

impl Default for AdapterExecutionEvidenceQueryRequest {
    fn default() -> Self {
        Self {
            group_id: None,
            domain: None,
            subject_id: None,
            adapter_id: None,
            source_id: None,
            execution_status: None,
            conformance_status: None,
            semantic_loss_status: None,
            after: None,
            max_items: default_max_items(),
            include_artifacts: false,
        }
    }
}

fn default_max_items() -> usize {
    100
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AdapterExecutionEvidenceJoinProjection {
    pub source_plan_digests: Vec<String>,
    pub intake_digests: Vec<String>,
    pub external_payload_digests: Vec<String>,
    pub workflow_reconciliation_digests: Vec<String>,
    pub missing_parent_digests: Vec<String>,
    pub unclassified_parent_digests: Vec<String>,
    pub source_bound: bool,
    pub workflow_bound: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AdapterExecutionEvidenceQueryRow {
    pub row_digest: String,
    pub content_digest: String,
    pub evidence_digest: String,
    pub subject_id: String,
    pub group_id: String,
    pub domains: Vec<String>,
    pub adapter_id: String,
    pub adapter_version: String,
    pub source_id: String,
    pub input_digest: String,
    pub output_digest: Option<String>,
    pub execution_status: String,
    pub conformance_status: String,
    pub semantic_loss_status: String,
    pub loss_count: usize,
    pub parent_digests: Vec<String>,
    pub attestation_posture: String,
    pub join_status: String,
    pub joins: AdapterExecutionEvidenceJoinProjection,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub evidence_artifact: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct AdapterExecutionEvidenceQuerySummary {
    pub page_row_count: usize,
    pub execution_status_counts: BTreeMap<String, usize>,
    pub conformance_status_counts: BTreeMap<String, usize>,
    pub semantic_loss_status_counts: BTreeMap<String, usize>,
    pub join_status_counts: BTreeMap<String, usize>,
    pub source_bound_rows: usize,
    pub workflow_bound_rows: usize,
    pub rows_with_missing_parents: usize,
    pub output_digest_present_rows: usize,
    pub total_loss_entries: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AdapterExecutionEvidenceQueryReport {
    pub ok: bool,
    pub schema: String,
    pub workflow: String,
    pub filters: AdapterExecutionEvidenceQueryRequest,
    pub registry_generation: u64,
    pub registry_size: usize,
    pub rows: Vec<AdapterExecutionEvidenceQueryRow>,
    pub page_summary: AdapterExecutionEvidenceQuerySummary,
    pub next_after: Option<String>,
    pub has_more: bool,
    pub query_digest: String,
    pub execution: String,
    pub readiness_claimed: bool,
    pub guarantees: Vec<String>,
    pub limitations: Vec<String>,
}

fn increment(counts: &mut BTreeMap<String, usize>, value: &str) {
    *counts.entry(value.to_string()).or_default() += 1;
}

fn summarize(rows: &[AdapterExecutionEvidenceQueryRow]) -> AdapterExecutionEvidenceQuerySummary {
    let mut summary = AdapterExecutionEvidenceQuerySummary {
        page_row_count: rows.len(),
        ..AdapterExecutionEvidenceQuerySummary::default()
    };
    for row in rows {
        increment(&mut summary.execution_status_counts, &row.execution_status);
        increment(
            &mut summary.conformance_status_counts,
            &row.conformance_status,
        );
        increment(
            &mut summary.semantic_loss_status_counts,
            &row.semantic_loss_status,
        );
        increment(&mut summary.join_status_counts, &row.join_status);
        if row.joins.source_bound {
            summary.source_bound_rows += 1;
        }
        if row.joins.workflow_bound {
            summary.workflow_bound_rows += 1;
        }
        if !row.joins.missing_parent_digests.is_empty() {
            summary.rows_with_missing_parents += 1;
        }
        if row.output_digest.is_some() {
            summary.output_digest_present_rows += 1;
        }
        summary.total_loss_entries += row.loss_count;
    }
    summary
}

fn validate_text(name: &str, value: &str) -> Result<(), String> {
    if value.trim().is_empty() || value.len() > 512 || value.chars().any(char::is_control) {
        return Err(format!(
            "{name} must be non-empty text of at most 512 bytes without control characters"
        ));
    }
    Ok(())
}

fn validate_digest(name: &str, value: &str) -> Result<(), String> {
    ContentHash::parse(value.to_string())
        .map(|_| ())
        .map_err(|_| format!("{name} must be a lowercase 64-character SHA-256 digest"))
}

fn validate_request(request: &AdapterExecutionEvidenceQueryRequest) -> Result<(), String> {
    if !(1..=MAX_ADAPTER_EXECUTION_EVIDENCE_QUERY_ITEMS).contains(&request.max_items) {
        return Err(format!(
            "max_items must be between 1 and {MAX_ADAPTER_EXECUTION_EVIDENCE_QUERY_ITEMS}"
        ));
    }
    for (name, value) in [
        ("group_id", request.group_id.as_ref()),
        ("domain", request.domain.as_ref()),
        ("subject_id", request.subject_id.as_ref()),
        ("adapter_id", request.adapter_id.as_ref()),
        ("source_id", request.source_id.as_ref()),
        ("execution_status", request.execution_status.as_ref()),
        ("conformance_status", request.conformance_status.as_ref()),
        (
            "semantic_loss_status",
            request.semantic_loss_status.as_ref(),
        ),
    ] {
        if let Some(value) = value {
            validate_text(name, value)?;
        }
    }
    if let Some(after) = request.after.as_deref() {
        validate_digest("after", after)?;
    }
    Ok(())
}

fn text(value: Option<&Value>) -> Option<String> {
    value.and_then(Value::as_str).map(ToOwned::to_owned)
}

fn digest(value: Option<&Value>) -> Option<String> {
    text(value).filter(|value| ContentHash::parse(value.to_owned()).is_ok())
}

fn text_array(value: Option<&Value>) -> Vec<String> {
    value
        .and_then(Value::as_array)
        .map(|values| {
            values
                .iter()
                .filter_map(|value| value.as_str().map(ToOwned::to_owned))
                .collect()
        })
        .unwrap_or_default()
}

fn parent_record<'a>(records: &'a [ArtifactRecord], digest: &str) -> Option<&'a ArtifactRecord> {
    records.iter().find(|record| {
        record.content_digest == digest || record.declared_digest.as_deref() == Some(digest)
    })
}

fn row_digest(row: &AdapterExecutionEvidenceQueryRow) -> Result<String, String> {
    let mut value = serde_json::to_value(row).map_err(|error| error.to_string())?;
    value
        .as_object_mut()
        .expect("adapter evidence query row serializes as an object")
        .remove("row_digest");
    ContentHash::of_value(&value)
        .map(|digest| digest.to_string())
        .map_err(|error| error.to_string())
}

fn joins_for(
    records: &[ArtifactRecord],
    parent_digests: &[String],
) -> AdapterExecutionEvidenceJoinProjection {
    let mut joins = AdapterExecutionEvidenceJoinProjection {
        source_plan_digests: Vec::new(),
        intake_digests: Vec::new(),
        external_payload_digests: Vec::new(),
        workflow_reconciliation_digests: Vec::new(),
        missing_parent_digests: Vec::new(),
        unclassified_parent_digests: Vec::new(),
        source_bound: false,
        workflow_bound: false,
    };
    for parent_digest in parent_digests {
        let Some(record) = parent_record(records, parent_digest) else {
            joins.missing_parent_digests.push(parent_digest.clone());
            continue;
        };
        match record.kind.as_str() {
            "domain_evidence_source_plan" => joins.source_plan_digests.push(parent_digest.clone()),
            "domain_evidence_intake" | "domain_report" | "domain_evidence_harmonization" => {
                joins.intake_digests.push(parent_digest.clone())
            }
            kind if kind.starts_with("domain_evidence_provider_external_payload") => {
                joins.external_payload_digests.push(parent_digest.clone())
            }
            "workflow_reconciliation" => joins
                .workflow_reconciliation_digests
                .push(parent_digest.clone()),
            _ => joins
                .unclassified_parent_digests
                .push(parent_digest.clone()),
        }
    }
    joins.source_bound = !joins.source_plan_digests.is_empty()
        || !joins.intake_digests.is_empty()
        || !joins.external_payload_digests.is_empty();
    joins.workflow_bound = !joins.workflow_reconciliation_digests.is_empty();
    joins
}

fn join_status(joins: &AdapterExecutionEvidenceJoinProjection) -> &'static str {
    if !joins.missing_parent_digests.is_empty() {
        "bound_with_missing_parents"
    } else if joins.source_bound && joins.workflow_bound {
        "source_and_workflow_bound"
    } else if joins.source_bound {
        "source_bound"
    } else if joins.workflow_bound {
        "workflow_bound"
    } else if !joins.unclassified_parent_digests.is_empty() {
        "parents_present_unclassified"
    } else {
        "unbound"
    }
}

/// Query retained adapter observations and classify only explicit digest parent joins.
pub fn query_adapter_execution_evidence(
    records: &[ArtifactRecord],
    generation: u64,
    request: AdapterExecutionEvidenceQueryRequest,
) -> Result<AdapterExecutionEvidenceQueryReport, String> {
    validate_request(&request)?;
    let mut rows = Vec::new();
    let mut has_more = false;
    for record in records.iter().filter(|record| {
        record.kind == "adapter_execution_evidence"
            && request
                .after
                .as_deref()
                .is_none_or(|after| record.content_digest.as_str() > after)
    }) {
        let artifact = &record.artifact;
        let matches =
            request.group_id.as_deref().is_none_or(|value| {
                artifact.get("group_id").and_then(Value::as_str) == Some(value)
            }) && request.domain.as_deref().is_none_or(|value| {
                artifact
                    .get("domains")
                    .and_then(Value::as_array)
                    .is_some_and(|domains| {
                        domains.iter().any(|domain| domain.as_str() == Some(value))
                    })
            }) && request.subject_id.as_deref().is_none_or(|value| {
                artifact.get("subject_id").and_then(Value::as_str) == Some(value)
            }) && request.adapter_id.as_deref().is_none_or(|value| {
                artifact.get("adapter_id").and_then(Value::as_str) == Some(value)
            }) && request.source_id.as_deref().is_none_or(|value| {
                artifact.get("source_id").and_then(Value::as_str) == Some(value)
            }) && request.execution_status.as_deref().is_none_or(|value| {
                artifact.get("execution_status").and_then(Value::as_str) == Some(value)
            }) && request.conformance_status.as_deref().is_none_or(|value| {
                artifact.get("conformance_status").and_then(Value::as_str) == Some(value)
            }) && request.semantic_loss_status.as_deref().is_none_or(|value| {
                artifact.get("semantic_loss_status").and_then(Value::as_str) == Some(value)
            });
        if !matches {
            continue;
        }
        if rows.len() >= request.max_items {
            has_more = true;
            break;
        }
        let parent_digests = text_array(artifact.get("parent_digests"));
        let joins = joins_for(records, &parent_digests);
        let mut row = AdapterExecutionEvidenceQueryRow {
            row_digest: String::new(),
            content_digest: record.content_digest.clone(),
            evidence_digest: digest(artifact.get("evidence_digest")).unwrap_or_default(),
            subject_id: text(artifact.get("subject_id")).unwrap_or_default(),
            group_id: text(artifact.get("group_id")).unwrap_or_default(),
            domains: text_array(artifact.get("domains")),
            adapter_id: text(artifact.get("adapter_id")).unwrap_or_default(),
            adapter_version: text(artifact.get("adapter_version")).unwrap_or_default(),
            source_id: text(artifact.get("source_id")).unwrap_or_default(),
            input_digest: digest(artifact.get("input_digest")).unwrap_or_default(),
            output_digest: digest(artifact.get("output_digest")),
            execution_status: text(artifact.get("execution_status")).unwrap_or_default(),
            conformance_status: text(artifact.get("conformance_status")).unwrap_or_default(),
            semantic_loss_status: text(artifact.get("semantic_loss_status")).unwrap_or_default(),
            loss_count: artifact
                .get("losses")
                .and_then(Value::as_array)
                .map_or(0, Vec::len),
            parent_digests,
            attestation_posture: text(artifact.get("attestation_posture")).unwrap_or_default(),
            join_status: join_status(&joins).into(),
            joins,
            evidence_artifact: request.include_artifacts.then(|| artifact.clone()),
        };
        row.row_digest = row_digest(&row)?;
        rows.push(row);
    }
    let next_after = if has_more {
        rows.last().map(|row| row.content_digest.clone())
    } else {
        None
    };
    let page_summary = summarize(&rows);
    let filters = serde_json::to_value(&request).map_err(|error| error.to_string())?;
    let query_digest = ContentHash::of_value(&json!({
        "schema": ADAPTER_EXECUTION_EVIDENCE_QUERY_SCHEMA,
        "workflow": ADAPTER_EXECUTION_EVIDENCE_QUERY_WORKFLOW,
        "filters": filters,
        "rows": rows,
    }))
    .map_err(|error| error.to_string())?
    .to_string();
    Ok(AdapterExecutionEvidenceQueryReport {
        ok: true,
        schema: ADAPTER_EXECUTION_EVIDENCE_QUERY_SCHEMA.into(),
        workflow: ADAPTER_EXECUTION_EVIDENCE_QUERY_WORKFLOW.into(),
        filters: request,
        registry_generation: generation,
        registry_size: records.len(),
        rows,
        page_summary,
        next_after,
        has_more,
        query_digest,
        execution: "not_started".into(),
        readiness_claimed: false,
        guarantees: vec![
            "rows are limited to digest-verified adapter_execution_evidence artifacts".into(),
            "source and workflow joins require explicit retained parent digests".into(),
            "cursoring and row identity are deterministic over the bounded local registry".into(),
        ],
        limitations: vec![
            "source_id, adapter labels, and domain overlap are not used to infer provenance or causality".into(),
            "missing parent records remain visible and are not treated as successful joins".into(),
            "the query never executes adapters, imports dependencies, fetches sources, or changes workflow state".into(),
            "a complete or verified join remains caller-asserted evidence and does not prove scientific, clinical, regulatory, release, or readiness validity".into(),
        ],
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record(kind: &str, declared_digest: Option<String>, artifact: Value) -> ArtifactRecord {
        ArtifactRecord {
            content_digest: format!("{}{}", "a".repeat(63), kind.len() % 10),
            kind: kind.into(),
            subject_id: "subject-1".into(),
            domains: vec!["genomics".into()],
            parent_digests: vec![],
            declared_digest,
            verification: json!({"state": "verified_integrity"}),
            artifact,
        }
    }

    #[test]
    fn query_classifies_explicit_source_and_workflow_parent_joins() {
        let source_digest = "b".repeat(64);
        let workflow_digest = "c".repeat(64);
        let evidence = json!({
            "evidence_digest": "d".repeat(64),
            "group_id": "biological_domains",
            "domains": ["genomics"],
            "subject_id": "subject-1",
            "adapter_id": "bioprism.python.vcf_text",
            "adapter_version": "0.1.0",
            "source_id": "vcf-1",
            "input_digest": "e".repeat(64),
            "output_digest": "f".repeat(64),
            "execution_status": "succeeded",
            "conformance_status": "verified",
            "semantic_loss_status": "lossless",
            "losses": [],
            "parent_digests": [source_digest.clone(), workflow_digest.clone()],
            "attestation_posture": "caller_asserted"
        });
        let records = vec![
            record(
                "domain_evidence_source_plan",
                Some(source_digest.clone()),
                json!({}),
            ),
            record(
                "workflow_reconciliation",
                Some(workflow_digest.clone()),
                json!({}),
            ),
            record("adapter_execution_evidence", None, evidence),
        ];
        let report = query_adapter_execution_evidence(
            &records,
            3,
            AdapterExecutionEvidenceQueryRequest::default(),
        )
        .unwrap();
        assert_eq!(report.rows.len(), 1);
        assert_eq!(report.rows[0].join_status, "source_and_workflow_bound");
        assert!(report.rows[0].joins.source_bound);
        assert!(report.rows[0].joins.workflow_bound);
        assert_eq!(report.page_summary.page_row_count, 1);
        assert_eq!(
            report.page_summary.join_status_counts["source_and_workflow_bound"],
            1
        );
        assert_eq!(report.page_summary.output_digest_present_rows, 1);
        assert!(!report.readiness_claimed);
    }

    #[test]
    fn query_keeps_missing_parent_and_cursor_posture_explicit() {
        let evidence = json!({
            "evidence_digest": "a".repeat(64),
            "group_id": "biological_domains",
            "domains": ["genomics"],
            "subject_id": "subject-1",
            "adapter_id": "bioprism.python.vcf_text",
            "adapter_version": "0.1.0",
            "source_id": "vcf-1",
            "input_digest": "b".repeat(64),
            "output_digest": null,
            "execution_status": "refused",
            "conformance_status": "refused",
            "semantic_loss_status": "unknown",
            "losses": [],
            "error_code": "missing_dependency",
            "parent_digests": ["c".repeat(64)],
            "attestation_posture": "caller_asserted"
        });
        let records = vec![record("adapter_execution_evidence", None, evidence)];
        let report = query_adapter_execution_evidence(
            &records,
            1,
            AdapterExecutionEvidenceQueryRequest {
                max_items: 1,
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(report.rows[0].join_status, "bound_with_missing_parents");
        assert!(!report.has_more);
        assert!(query_adapter_execution_evidence(
            &records,
            1,
            AdapterExecutionEvidenceQueryRequest {
                after: Some("not-a-digest".into()),
                ..Default::default()
            }
        )
        .is_err());
    }
}
