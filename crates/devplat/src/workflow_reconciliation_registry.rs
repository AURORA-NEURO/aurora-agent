//! Bounded durable indexing for domain-workflow reconciliation reports.
//!
//! A reconciliation report is a derived audit projection, not a mission checkpoint and not a
//! portable evidence bundle. This registry keeps that distinction explicit: it stores only
//! reports produced by the digest-bound reconciliation kernel, indexes the fields operators need
//! to find a review, and re-verifies both every report digest and the registry snapshot digest on
//! restore. Importing and querying never dispatches, retries, resumes, or mutates a mission.

use bioprism_ids::ContentHash;
use serde_json::{json, Value};
use std::collections::BTreeMap;
use thiserror::Error;

pub const DOMAIN_WORKFLOW_RECONCILIATION_REGISTRY_SCHEMA_VERSION: &str =
    "bioprism-devplat-domain-workflow-reconciliation-registry/0.1";
pub const DOMAIN_WORKFLOW_RECONCILIATION_IMPORT_SCHEMA_VERSION: &str =
    "bioprism-devplat-domain-workflow-reconciliation-import/0.1";
pub const DOMAIN_WORKFLOW_RECONCILIATION_QUERY_SCHEMA_VERSION: &str =
    "bioprism-devplat-domain-workflow-reconciliation-query/0.1";
pub const DOMAIN_WORKFLOW_RECONCILIATION_SUMMARY_SCHEMA_VERSION: &str =
    "bioprism-devplat-domain-workflow-reconciliation-summary/0.1";
pub const MAX_DOMAIN_WORKFLOW_RECONCILIATIONS: usize = 512;
pub const MAX_DOMAIN_WORKFLOW_RECONCILIATION_BYTES: usize = 32 * 1024 * 1024;
pub const MAX_DOMAIN_WORKFLOW_RECONCILIATION_QUERY_ITEMS: usize = 256;

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum DomainWorkflowReconciliationRegistryError {
    #[error("workflow reconciliation registry input is not an object")]
    NotObject,
    #[error("workflow reconciliation record is invalid: {0}")]
    InvalidRecord(String),
    #[error("workflow reconciliation registry has reached its {maximum}-record limit")]
    Full { maximum: usize },
    #[error("workflow reconciliation registry snapshot is invalid: {0}")]
    InvalidSnapshot(String),
    #[error(
        "workflow reconciliation registry snapshot is {actual} bytes, above the {maximum}-byte bound"
    )]
    SnapshotTooLarge { actual: usize, maximum: usize },
    #[error("workflow reconciliation registry could not be canonicalised: {0}")]
    Canonicalisation(String),
}

#[derive(Debug, Clone, Default)]
pub struct DomainWorkflowReconciliationRegistry {
    generation: u64,
    records: BTreeMap<String, Value>,
}

impl DomainWorkflowReconciliationRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn generation(&self) -> u64 {
        self.generation
    }

    pub fn len(&self) -> usize {
        self.records.len()
    }

    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }

    /// Import one kernel-produced report. Re-importing the identical report is idempotent.
    pub fn import(
        &mut self,
        record: &Value,
    ) -> Result<Value, DomainWorkflowReconciliationRegistryError> {
        let normalized = normalized_record(record)?;
        let digest = verify_record(&normalized)?;
        let already_present = self
            .records
            .get(&digest)
            .is_some_and(|existing| existing == &normalized);
        if !already_present && self.records.len() >= MAX_DOMAIN_WORKFLOW_RECONCILIATIONS {
            return Err(DomainWorkflowReconciliationRegistryError::Full {
                maximum: MAX_DOMAIN_WORKFLOW_RECONCILIATIONS,
            });
        }
        if !already_present {
            let mut candidate = self.clone();
            candidate.records.insert(digest.clone(), normalized);
            candidate.generation = candidate.generation.saturating_add(1);
            candidate.ensure_snapshot_bound()?;
            self.records = candidate.records;
            self.generation = candidate.generation;
        }
        Ok(json!({
            "ok": true,
            "schema": DOMAIN_WORKFLOW_RECONCILIATION_IMPORT_SCHEMA_VERSION,
            "workflow": "domain_workflow_reconciliation_import",
            "reconciliation_digest": digest,
            "created": !already_present,
            "already_present": already_present,
            "registry_generation": self.generation,
            "registry_size": self.records.len(),
            "execution": "not_started",
            "guarantees": [
                "only a digest-valid domain_workflow_reconcile report is indexed",
                "re-importing the same canonical report is idempotent",
                "import does not execute, retry, resume, or mutate a mission"
            ],
            "limitations": [
                "the registry is a bounded local audit index rather than a distributed store",
                "a complete reconciliation remains review-required and non-claiming"
            ]
        }))
    }

    pub fn get(&self, digest: &str) -> Option<Value> {
        self.records.get(digest).cloned()
    }

    /// Build a compact operator projection without returning report bodies.
    ///
    /// This is derived from the same retained records as `query`, making the operator snapshot
    /// useful after restart while keeping readiness, evidence validity, and integrity separate.
    /// No counter in this projection is an execution or domain-success claim.
    pub fn operator_summary(&self) -> Value {
        let mut completion_status_counts = BTreeMap::<String, usize>::new();
        let mut workflow_status_counts = BTreeMap::<String, BTreeMap<String, usize>>::new();
        let mut ready_count = 0usize;
        let mut review_required_count = 0usize;
        let mut integrity_invalid_count = 0usize;
        let mut evidence_invalid_count = 0usize;
        for record in self.records.values() {
            let status = record
                .pointer("/completion/status")
                .and_then(Value::as_str)
                .unwrap_or("unknown")
                .to_string();
            *completion_status_counts.entry(status).or_default() += 1;
            let workflow_id = record
                .get("workflow_id")
                .and_then(Value::as_str)
                .unwrap_or("unknown")
                .to_string();
            *workflow_status_counts
                .entry(workflow_id)
                .or_default()
                .entry(
                    record
                        .pointer("/completion/status")
                        .and_then(Value::as_str)
                        .unwrap_or("unknown")
                        .to_string(),
                )
                .or_default() += 1;
            if record.pointer("/completion/ready").and_then(Value::as_bool) == Some(true) {
                ready_count += 1;
            }
            if record
                .pointer("/completion/review_required")
                .and_then(Value::as_bool)
                == Some(true)
            {
                review_required_count += 1;
            }
            if record.pointer("/integrity/valid").and_then(Value::as_bool) != Some(true) {
                integrity_invalid_count += 1;
            }
            if record
                .pointer("/evidence/evidence_valid")
                .and_then(Value::as_bool)
                != Some(true)
            {
                evidence_invalid_count += 1;
            }
        }
        json!({
            "ok": true,
            "schema": DOMAIN_WORKFLOW_RECONCILIATION_SUMMARY_SCHEMA_VERSION,
            "workflow": "domain_workflow_reconciliation_summary",
            "registry_generation": self.generation,
            "registry_size": self.records.len(),
            "completion_status_counts": completion_status_counts,
            "workflow_count": workflow_status_counts.len(),
            "workflow_status_counts": workflow_status_counts,
            "ready_count": ready_count,
            "review_required_count": review_required_count,
            "integrity_invalid_count": integrity_invalid_count,
            "evidence_invalid_count": evidence_invalid_count,
            "retention": {
                "max_reconciliations": MAX_DOMAIN_WORKFLOW_RECONCILIATIONS,
                "max_bytes": MAX_DOMAIN_WORKFLOW_RECONCILIATION_BYTES
            },
            "execution": "not_started",
            "readiness_claimed": false,
            "guarantees": [
                "counts are derived from stored digest-valid reconciliation reports",
                "completion, integrity, and evidence counters remain separate",
                "summary generation does not execute, retry, resume, or re-evaluate a mission"
            ],
            "limitations": [
                "the summary covers only the bounded local registry",
                "a ready count is structural evidence posture and not a domain-success claim"
            ]
        })
    }

    /// Return the bounded reconciliation posture for one capability-group workflow.
    ///
    /// A missing record is reported as `missing`, while an explicitly incomplete or invalid
    /// retained report is surfaced as a blocking audit posture. A structurally ready record is
    /// still only evidence for review; it never becomes a gate pass by itself.
    pub fn workflow_posture(&self, workflow_id: &str) -> Value {
        let mut completion_status_counts = BTreeMap::<String, usize>::new();
        let mut record_count = 0usize;
        let mut ready_count = 0usize;
        let mut review_required_count = 0usize;
        let mut integrity_invalid_count = 0usize;
        let mut evidence_invalid_count = 0usize;
        for record in self
            .records
            .values()
            .filter(|record| record.get("workflow_id").and_then(Value::as_str) == Some(workflow_id))
        {
            record_count += 1;
            let status = record
                .pointer("/completion/status")
                .and_then(Value::as_str)
                .unwrap_or("unknown")
                .to_string();
            *completion_status_counts.entry(status).or_default() += 1;
            if record.pointer("/completion/ready").and_then(Value::as_bool) == Some(true) {
                ready_count += 1;
            }
            if record
                .pointer("/completion/review_required")
                .and_then(Value::as_bool)
                == Some(true)
            {
                review_required_count += 1;
            }
            if record.pointer("/integrity/valid").and_then(Value::as_bool) != Some(true) {
                integrity_invalid_count += 1;
            }
            if record
                .pointer("/evidence/evidence_valid")
                .and_then(Value::as_bool)
                != Some(true)
            {
                evidence_invalid_count += 1;
            }
        }
        let state = if record_count == 0 {
            "missing"
        } else if integrity_invalid_count > 0 || evidence_invalid_count > 0 {
            "invalid"
        } else if ready_count > 0 {
            "structurally_ready"
        } else {
            "incomplete"
        };
        json!({
            "workflow_id": workflow_id,
            "state": state,
            "record_count": record_count,
            "completion_status_counts": completion_status_counts,
            "ready_count": ready_count,
            "review_required_count": review_required_count,
            "integrity_invalid_count": integrity_invalid_count,
            "evidence_invalid_count": evidence_invalid_count,
            "readiness_claimed": false,
            "scope": "bounded_digest_valid_reconciliation_registry",
            "guarantees": [
                "only records whose reconciliation_digest passed import verification are counted",
                "structurally_ready is evidence posture and still requires human or domain authority review",
                "posture lookup never executes, retries, resumes, or re-evaluates a mission"
            ],
            "limitations": [
                "missing means no matching retained record, not that the workflow never ran",
                "the registry is bounded and process-local"
            ]
        })
    }

    /// Query deterministic digest-ordered index rows without returning full reports by default.
    #[allow(clippy::too_many_arguments)]
    pub fn query(
        &self,
        mission_id: Option<&str>,
        workflow_id: Option<&str>,
        mission_plan_digest: Option<&str>,
        completion_status: Option<&str>,
        after: Option<&str>,
        max_items: usize,
        include_records: bool,
    ) -> Result<Value, DomainWorkflowReconciliationRegistryError> {
        if !(1..=MAX_DOMAIN_WORKFLOW_RECONCILIATION_QUERY_ITEMS).contains(&max_items) {
            return Err(DomainWorkflowReconciliationRegistryError::InvalidSnapshot(
                format!(
                    "max_items must be between 1 and {MAX_DOMAIN_WORKFLOW_RECONCILIATION_QUERY_ITEMS}"
                ),
            ));
        }
        let mut rows = Vec::new();
        let mut has_more = false;
        for (digest, record) in self
            .records
            .iter()
            .filter(|(digest, _)| after.is_none_or(|cursor| digest.as_str() > cursor))
        {
            let index = index_row(digest, record)?;
            let matches =
                mission_id.is_none_or(|value| {
                    index.get("mission_id").and_then(Value::as_str) == Some(value)
                }) && workflow_id.is_none_or(|value| {
                    index.get("workflow_id").and_then(Value::as_str) == Some(value)
                }) && mission_plan_digest.is_none_or(|value| {
                    index.get("mission_plan_digest").and_then(Value::as_str) == Some(value)
                }) && completion_status.is_none_or(|value| {
                    index.get("completion_status").and_then(Value::as_str) == Some(value)
                });
            if !matches {
                continue;
            }
            if rows.len() >= max_items {
                has_more = true;
                break;
            }
            let mut row = index;
            if include_records {
                row["record"] = record.clone();
            }
            rows.push(row);
        }
        let next_after = if has_more {
            rows.last()
                .and_then(|row| row.get("reconciliation_digest"))
                .cloned()
                .unwrap_or(Value::Null)
        } else {
            Value::Null
        };
        Ok(json!({
            "ok": true,
            "schema": DOMAIN_WORKFLOW_RECONCILIATION_QUERY_SCHEMA_VERSION,
            "workflow": "domain_workflow_reconciliation_query",
            "filters": {
                "mission_id": mission_id,
                "workflow_id": workflow_id,
                "mission_plan_digest": mission_plan_digest,
                "completion_status": completion_status,
                "after": after,
                "max_items": max_items,
                "include_records": include_records
            },
            "registry_generation": self.generation,
            "registry_size": self.records.len(),
            "rows": rows,
            "next_after": next_after,
            "has_more": has_more,
            "execution": "not_started",
            "guarantees": [
                "rows are ordered by reconciliation digest",
                "filters are applied to stored structural identity and completion posture",
                "query does not execute a mission, evaluator, domain tool, or external effect"
            ],
            "limitations": [
                "results are bounded by local registry retention",
                "absence from this registry is not evidence that a reconciliation never existed"
            ]
        }))
    }

    /// Return a digest-protected checkpoint suitable for atomic persistence.
    pub fn snapshot(&self) -> Result<Value, DomainWorkflowReconciliationRegistryError> {
        let mut document = json!({
            "schema": DOMAIN_WORKFLOW_RECONCILIATION_REGISTRY_SCHEMA_VERSION,
            "generation": self.generation,
            "reconciliation_count": self.records.len(),
            "reconciliations": self.records.iter().map(|(digest, record)| json!({
                "reconciliation_digest": digest,
                "record": record
            })).collect::<Vec<_>>(),
            "retention": {
                "max_reconciliations": MAX_DOMAIN_WORKFLOW_RECONCILIATIONS,
                "max_bytes": MAX_DOMAIN_WORKFLOW_RECONCILIATION_BYTES
            },
            "execution": "not_started"
        });
        let state_digest = snapshot_digest(&document)?;
        document["state_digest"] = Value::String(state_digest);
        self.ensure_encoded_bound(&document)?;
        Ok(document)
    }

    /// Restore a registry while re-verifying every record and the registry digest.
    pub fn from_snapshot(
        document: &Value,
    ) -> Result<Self, DomainWorkflowReconciliationRegistryError> {
        let object = document.as_object().ok_or_else(|| {
            DomainWorkflowReconciliationRegistryError::InvalidSnapshot(
                "snapshot must be an object".into(),
            )
        })?;
        let encoded = serde_json::to_vec(document).map_err(|error| {
            DomainWorkflowReconciliationRegistryError::Canonicalisation(error.to_string())
        })?;
        if encoded.len() > MAX_DOMAIN_WORKFLOW_RECONCILIATION_BYTES {
            return Err(
                DomainWorkflowReconciliationRegistryError::SnapshotTooLarge {
                    actual: encoded.len(),
                    maximum: MAX_DOMAIN_WORKFLOW_RECONCILIATION_BYTES,
                },
            );
        }
        if object.get("schema").and_then(Value::as_str)
            != Some(DOMAIN_WORKFLOW_RECONCILIATION_REGISTRY_SCHEMA_VERSION)
        {
            return Err(DomainWorkflowReconciliationRegistryError::InvalidSnapshot(
                "schema is invalid".into(),
            ));
        }
        let claimed_digest = object
            .get("state_digest")
            .and_then(Value::as_str)
            .ok_or_else(|| {
                DomainWorkflowReconciliationRegistryError::InvalidSnapshot(
                    "state_digest is missing".into(),
                )
            })?;
        let mut unsigned = document.clone();
        unsigned
            .as_object_mut()
            .expect("snapshot object was checked above")
            .remove("state_digest");
        if claimed_digest != snapshot_digest(&unsigned)? {
            return Err(DomainWorkflowReconciliationRegistryError::InvalidSnapshot(
                "state_digest does not match snapshot contents".into(),
            ));
        }
        let generation = object
            .get("generation")
            .and_then(Value::as_u64)
            .ok_or_else(|| {
                DomainWorkflowReconciliationRegistryError::InvalidSnapshot(
                    "generation is invalid".into(),
                )
            })?;
        let rows = object
            .get("reconciliations")
            .and_then(Value::as_array)
            .ok_or_else(|| {
                DomainWorkflowReconciliationRegistryError::InvalidSnapshot(
                    "reconciliations must be an array".into(),
                )
            })?;
        if rows.len() > MAX_DOMAIN_WORKFLOW_RECONCILIATIONS {
            return Err(DomainWorkflowReconciliationRegistryError::Full {
                maximum: MAX_DOMAIN_WORKFLOW_RECONCILIATIONS,
            });
        }
        let mut registry = Self {
            generation,
            records: BTreeMap::new(),
        };
        for row in rows {
            let row_object = row.as_object().ok_or_else(|| {
                DomainWorkflowReconciliationRegistryError::InvalidSnapshot(
                    "reconciliation index row must be an object".into(),
                )
            })?;
            let claimed = row_object
                .get("reconciliation_digest")
                .and_then(Value::as_str)
                .ok_or_else(|| {
                    DomainWorkflowReconciliationRegistryError::InvalidSnapshot(
                        "reconciliation_digest is missing".into(),
                    )
                })?;
            let record = row_object.get("record").ok_or_else(|| {
                DomainWorkflowReconciliationRegistryError::InvalidSnapshot(
                    "reconciliation record body is missing".into(),
                )
            })?;
            let recomputed = verify_record(record).map_err(|error| {
                DomainWorkflowReconciliationRegistryError::InvalidSnapshot(format!(
                    "reconciliation {claimed} is invalid: {error}"
                ))
            })?;
            if recomputed != claimed {
                return Err(DomainWorkflowReconciliationRegistryError::InvalidSnapshot(
                    format!("reconciliation {claimed} failed digest verification"),
                ));
            }
            if registry
                .records
                .insert(claimed.to_string(), record.clone())
                .is_some()
            {
                return Err(DomainWorkflowReconciliationRegistryError::InvalidSnapshot(
                    "snapshot contains duplicate reconciliation digests".into(),
                ));
            }
        }
        if object.get("reconciliation_count").and_then(Value::as_u64) != Some(rows.len() as u64) {
            return Err(DomainWorkflowReconciliationRegistryError::InvalidSnapshot(
                "reconciliation_count does not match reconciliations".into(),
            ));
        }
        registry.ensure_snapshot_bound()?;
        Ok(registry)
    }

    fn ensure_snapshot_bound(&self) -> Result<(), DomainWorkflowReconciliationRegistryError> {
        let document = self.snapshot()?;
        self.ensure_encoded_bound(&document)
    }

    fn ensure_encoded_bound(
        &self,
        document: &Value,
    ) -> Result<(), DomainWorkflowReconciliationRegistryError> {
        let bytes = serde_json::to_vec(document).map_err(|error| {
            DomainWorkflowReconciliationRegistryError::Canonicalisation(error.to_string())
        })?;
        if bytes.len() > MAX_DOMAIN_WORKFLOW_RECONCILIATION_BYTES {
            return Err(
                DomainWorkflowReconciliationRegistryError::SnapshotTooLarge {
                    actual: bytes.len(),
                    maximum: MAX_DOMAIN_WORKFLOW_RECONCILIATION_BYTES,
                },
            );
        }
        Ok(())
    }
}

fn verify_record(record: &Value) -> Result<String, DomainWorkflowReconciliationRegistryError> {
    let object = record
        .as_object()
        .ok_or(DomainWorkflowReconciliationRegistryError::NotObject)?;
    if object.get("ok").and_then(Value::as_bool) != Some(true) {
        return Err(DomainWorkflowReconciliationRegistryError::InvalidRecord(
            "ok must be true".into(),
        ));
    }
    if object.get("workflow").and_then(Value::as_str) != Some("domain_workflow_reconcile") {
        return Err(DomainWorkflowReconciliationRegistryError::InvalidRecord(
            "workflow must be domain_workflow_reconcile".into(),
        ));
    }
    for field in [
        "workflow_id",
        "workflow_digest",
        "catalog_digest",
        "domain_contract_digest",
        "mission_id",
        "mission_plan_digest",
        "reconciliation_digest",
    ] {
        if object
            .get(field)
            .and_then(Value::as_str)
            .is_none_or(|value| value.trim().is_empty())
        {
            return Err(DomainWorkflowReconciliationRegistryError::InvalidRecord(
                format!("{field} must be a non-empty string"),
            ));
        }
    }
    let claimed = object
        .get("reconciliation_digest")
        .and_then(Value::as_str)
        .expect("reconciliation_digest was checked above");
    ContentHash::parse(claimed.to_string()).map_err(|_| {
        DomainWorkflowReconciliationRegistryError::InvalidRecord(
            "reconciliation_digest must be a lowercase SHA-256 content hash".into(),
        )
    })?;
    let mut unsigned = record.clone();
    unsigned
        .as_object_mut()
        .expect("record object was checked above")
        .remove("reconciliation_digest");
    let recomputed = ContentHash::of_value(&unsigned)
        .map_err(|error| {
            DomainWorkflowReconciliationRegistryError::Canonicalisation(error.to_string())
        })?
        .to_string();
    if recomputed != claimed {
        return Err(DomainWorkflowReconciliationRegistryError::InvalidRecord(
            format!("reconciliation_digest does not match record contents: expected {claimed}, computed {recomputed}"),
        ));
    }
    Ok(claimed.to_string())
}

fn normalized_record(record: &Value) -> Result<Value, DomainWorkflowReconciliationRegistryError> {
    if !record.is_object() {
        return Err(DomainWorkflowReconciliationRegistryError::NotObject);
    }
    let mut normalized = record.clone();
    let object = normalized
        .as_object_mut()
        .expect("record object was checked above");
    object.remove("request_id");
    object.remove("__isError");
    Ok(normalized)
}

fn snapshot_digest(document: &Value) -> Result<String, DomainWorkflowReconciliationRegistryError> {
    ContentHash::of_value(document)
        .map(|digest| digest.to_string())
        .map_err(|error| {
            DomainWorkflowReconciliationRegistryError::Canonicalisation(error.to_string())
        })
}

fn index_row(
    digest: &str,
    record: &Value,
) -> Result<Value, DomainWorkflowReconciliationRegistryError> {
    let object = record
        .as_object()
        .ok_or(DomainWorkflowReconciliationRegistryError::NotObject)?;
    Ok(json!({
        "reconciliation_digest": digest,
        "workflow_id": object.get("workflow_id").cloned().unwrap_or(Value::Null),
        "workflow_digest": object.get("workflow_digest").cloned().unwrap_or(Value::Null),
        "catalog_digest": object.get("catalog_digest").cloned().unwrap_or(Value::Null),
        "domain_contract_digest": object.get("domain_contract_digest").cloned().unwrap_or(Value::Null),
        "mission_id": object.get("mission_id").cloned().unwrap_or(Value::Null),
        "mission_plan_digest": object.get("mission_plan_digest").cloned().unwrap_or(Value::Null),
        "source": object.get("source").cloned().unwrap_or(Value::Null),
        "completion_status": record.pointer("/completion/status").cloned().unwrap_or(Value::Null),
        "ready": record.pointer("/completion/ready").cloned().unwrap_or(Value::Null),
        "review_required": record.pointer("/completion/review_required").cloned().unwrap_or(Value::Null),
        "integrity_valid": record.pointer("/integrity/valid").cloned().unwrap_or(Value::Null),
        "evidence_valid": record.pointer("/evidence/evidence_valid").cloned().unwrap_or(Value::Null),
        "finding_count": record.pointer("/integrity/finding_count").cloned().unwrap_or(Value::Null),
        "execution": object.get("execution").cloned().unwrap_or(Value::Null)
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record(mission_id: &str, workflow_id: &str, status: &str) -> Value {
        let mut record = json!({
            "ok": true,
            "schema": "bioprism-devplat-domain-workflow-reconcile/0.1",
            "workflow": "domain_workflow_reconcile",
            "workflow_id": workflow_id,
            "workflow_digest": "a".repeat(64),
            "catalog_digest": "b".repeat(64),
            "domain_contract_digest": "c".repeat(64),
            "mission_id": mission_id,
            "mission_plan_digest": "d".repeat(64),
            "source": "mission_report",
            "completion": {"status": status, "ready": status == "complete", "review_required": true},
            "evidence": {"evidence_valid": status == "complete"},
            "integrity": {"valid": true, "finding_count": 0},
            "execution": "not_started"
        });
        let digest = ContentHash::of_value(&record).unwrap().to_string();
        record["reconciliation_digest"] = Value::String(digest);
        record
    }

    #[test]
    fn imports_idempotently_and_filters_structural_posture() {
        let mut registry = DomainWorkflowReconciliationRegistry::new();
        let first_record = record("mission-one", "oncology", "complete");
        let first = registry.import(&first_record).unwrap();
        assert_eq!(first["created"], true);
        let second = registry.import(&first_record).unwrap();
        assert_eq!(second["already_present"], true);
        let query = registry
            .query(
                Some("mission-one"),
                Some("oncology"),
                None,
                Some("complete"),
                None,
                10,
                false,
            )
            .unwrap();
        assert_eq!(query["rows"].as_array().unwrap().len(), 1);
        assert_eq!(query["rows"][0]["ready"], true);
        let summary = registry.operator_summary();
        assert_eq!(summary["registry_size"], 1);
        assert_eq!(summary["ready_count"], 1);
        assert_eq!(summary["review_required_count"], 1);
        assert_eq!(summary["completion_status_counts"]["complete"], 1);
        assert_eq!(summary["workflow_count"], 1);
        assert_eq!(summary["workflow_status_counts"]["oncology"]["complete"], 1);
        assert_eq!(
            registry.workflow_posture("oncology")["state"],
            "structurally_ready"
        );
        assert_eq!(registry.workflow_posture("missing")["state"], "missing");
    }

    #[test]
    fn snapshot_round_trip_reverifies_record_and_state_digests() {
        let mut registry = DomainWorkflowReconciliationRegistry::new();
        registry
            .import(&record("mission-one", "workspace", "partial"))
            .unwrap();
        let snapshot = registry.snapshot().unwrap();
        let restored = DomainWorkflowReconciliationRegistry::from_snapshot(&snapshot).unwrap();
        assert_eq!(restored.len(), 1);
        assert_eq!(restored.generation(), 1);

        let mut tampered = snapshot;
        tampered["reconciliations"][0]["record"]["completion"]["status"] = json!("complete");
        assert!(matches!(
            DomainWorkflowReconciliationRegistry::from_snapshot(&tampered),
            Err(DomainWorkflowReconciliationRegistryError::InvalidSnapshot(
                _
            ))
        ));
    }

    #[test]
    fn query_is_digest_ordered_and_cursor_bounded() {
        let mut registry = DomainWorkflowReconciliationRegistry::new();
        registry
            .import(&record("mission-one", "workspace", "complete"))
            .unwrap();
        registry
            .import(&record("mission-two", "oncology", "failed"))
            .unwrap();
        let all = registry
            .query(None, None, None, None, None, 1, false)
            .unwrap();
        assert_eq!(all["rows"].as_array().unwrap().len(), 1);
        assert_eq!(all["has_more"], true);
        let cursor = all["next_after"].as_str().unwrap();
        let next = registry
            .query(None, None, None, None, Some(cursor), 10, false)
            .unwrap();
        assert_eq!(next["rows"].as_array().unwrap().len(), 1);
        assert_eq!(next["has_more"], false);
        let summary = registry.operator_summary();
        assert_eq!(summary["workflow_count"], 2);
        assert_eq!(summary["workflow_status_counts"]["oncology"]["failed"], 1);
        assert_eq!(
            summary["workflow_status_counts"]["workspace"]["complete"],
            1
        );
        assert_eq!(registry.workflow_posture("oncology")["state"], "invalid");
    }
}
