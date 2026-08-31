//! Bounded durable indexing for domain-workflow reconciliation reports.
//!
//! A reconciliation report is a derived audit projection, not a mission checkpoint and not a
//! portable evidence bundle. This registry keeps that distinction explicit: it stores only
//! reports produced by the digest-bound reconciliation kernel, indexes the fields operators need
//! to find a review, and re-verifies both every report digest and the registry snapshot digest on
//! restore. Importing and querying never dispatches, retries, resumes, or mutates a mission.

use bioprism_ids::ContentHash;
use serde_json::{json, Map, Value};
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
const MAX_TEXT_BYTES: usize = 4_096;
const MAX_FINDINGS: usize = 512;

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum DomainWorkflowReconciliationRegistryError {
    #[error("workflow reconciliation registry input is not an object")]
    NotObject,
    #[error("workflow reconciliation record is invalid: {0}")]
    InvalidRecord(String),
    #[error("workflow reconciliation registry has reached its {maximum}-record limit")]
    Full { maximum: usize },
    #[error("workflow reconciliation registry generation counter is exhausted")]
    GenerationExhausted,
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

    /// Return deterministic reconciliation digest identities without exposing report bodies.
    pub fn digests_for_audit(&self) -> Vec<String> {
        self.records.keys().cloned().collect()
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
            candidate.generation = candidate
                .generation
                .checked_add(1)
                .ok_or(DomainWorkflowReconciliationRegistryError::GenerationExhausted)?;
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
        valid_digest(digest)
            .then(|| self.records.get(digest).cloned())
            .flatten()
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
        } else if integrity_invalid_count > 0 {
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
        decision_readiness_state: Option<&str>,
        decision_readiness_gate_satisfied: Option<bool>,
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
        if let Some(value) = mission_id {
            if !valid_identifier(value) {
                return Err(DomainWorkflowReconciliationRegistryError::InvalidSnapshot(
                    "mission_id filter must be a bounded visible identifier".into(),
                ));
            }
        }
        if let Some(value) = workflow_id {
            if !valid_identifier(value) {
                return Err(DomainWorkflowReconciliationRegistryError::InvalidSnapshot(
                    "workflow_id filter must be a bounded visible identifier".into(),
                ));
            }
        }
        if let Some(value) = mission_plan_digest {
            if !valid_digest(value) {
                return Err(DomainWorkflowReconciliationRegistryError::InvalidSnapshot(
                    "mission_plan_digest filter must be a canonical lowercase content hash".into(),
                ));
            }
        }
        if let Some(value) = completion_status {
            if !matches!(
                value,
                "complete" | "complete_with_output_omissions" | "partial" | "failed" | "unverified"
            ) {
                return Err(DomainWorkflowReconciliationRegistryError::InvalidSnapshot(
                    "completion_status filter is not a recognized reconciliation status".into(),
                ));
            }
        }
        if let Some(value) = decision_readiness_state {
            if !valid_text(value) {
                return Err(DomainWorkflowReconciliationRegistryError::InvalidSnapshot(
                    "decision_readiness_state filter must be bounded visible text".into(),
                ));
            }
        }
        if let Some(value) = after {
            if !valid_digest(value) {
                return Err(DomainWorkflowReconciliationRegistryError::InvalidSnapshot(
                    "after cursor must be a canonical lowercase content hash".into(),
                ));
            }
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
                }) && decision_readiness_state.is_none_or(|value| {
                    index
                        .get("decision_readiness_state")
                        .and_then(Value::as_str)
                        == Some(value)
                }) && decision_readiness_gate_satisfied.is_none_or(|value| {
                    index
                        .get("decision_readiness_gate_satisfied")
                        .and_then(Value::as_bool)
                        == Some(value)
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
                "decision_readiness_state": decision_readiness_state,
                "decision_readiness_gate_satisfied": decision_readiness_gate_satisfied,
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
        if object.get("execution").and_then(Value::as_str) != Some("not_started") {
            return Err(DomainWorkflowReconciliationRegistryError::InvalidSnapshot(
                "execution must remain not_started".into(),
            ));
        }
        let expected_retention = json!({
            "max_reconciliations": MAX_DOMAIN_WORKFLOW_RECONCILIATIONS,
            "max_bytes": MAX_DOMAIN_WORKFLOW_RECONCILIATION_BYTES
        });
        if object.get("retention") != Some(&expected_retention) {
            return Err(DomainWorkflowReconciliationRegistryError::InvalidSnapshot(
                "retention contract does not match the registry bounds".into(),
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
            .ok_or_else(|| {
                DomainWorkflowReconciliationRegistryError::InvalidSnapshot(
                    "snapshot must remain an object while removing state_digest".into(),
                )
            })?
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
        if generation < rows.len() as u64 {
            return Err(DomainWorkflowReconciliationRegistryError::InvalidSnapshot(
                "generation cannot be below the retained reconciliation count".into(),
            ));
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

fn verify_decision_readiness(
    object: &Map<String, Value>,
) -> Result<(), DomainWorkflowReconciliationRegistryError> {
    let Some(readiness) = object.get("decision_readiness") else {
        return Ok(());
    };
    let readiness = readiness.as_object().ok_or_else(|| {
        DomainWorkflowReconciliationRegistryError::InvalidRecord(
            "decision_readiness must be an object".into(),
        )
    })?;
    let required = readiness
        .get("required")
        .and_then(Value::as_bool)
        .ok_or_else(|| {
            DomainWorkflowReconciliationRegistryError::InvalidRecord(
                "decision_readiness.required must be a boolean".into(),
            )
        })?;
    let provided = readiness
        .get("provided")
        .and_then(Value::as_bool)
        .ok_or_else(|| {
            DomainWorkflowReconciliationRegistryError::InvalidRecord(
                "decision_readiness.provided must be a boolean".into(),
            )
        })?;
    let policy_satisfied = readiness
        .get("policy_satisfied")
        .and_then(Value::as_bool)
        .ok_or_else(|| {
            DomainWorkflowReconciliationRegistryError::InvalidRecord(
                "decision_readiness.policy_satisfied must be a boolean".into(),
            )
        })?;
    let gate_satisfied = readiness
        .get("gate_satisfied")
        .and_then(Value::as_bool)
        .ok_or_else(|| {
            DomainWorkflowReconciliationRegistryError::InvalidRecord(
                "decision_readiness.gate_satisfied must be a boolean".into(),
            )
        })?;
    if gate_satisfied != (!required || policy_satisfied)
        || object
            .get("decision_review_gate_satisfied")
            .and_then(Value::as_bool)
            != Some(gate_satisfied)
    {
        return Err(DomainWorkflowReconciliationRegistryError::InvalidRecord(
            "decision readiness gate does not match its policy and top-level projection".into(),
        ));
    }
    if readiness.get("readiness_claimed") != Some(&Value::Bool(false))
        || readiness.get("execution").and_then(Value::as_str) != Some("not_started")
    {
        return Err(DomainWorkflowReconciliationRegistryError::InvalidRecord(
            "decision readiness must remain non-claiming and not_started".into(),
        ));
    }
    let audit_digest = readiness.get("audit_digest");
    let decision_state = readiness.get("decision_state");
    let subject_id = readiness.get("subject_id");
    if provided {
        if audit_digest
            .and_then(Value::as_str)
            .is_none_or(|value| !valid_digest(value))
            || decision_state
                .and_then(Value::as_str)
                .is_none_or(|value| !valid_text(value))
            || subject_id
                .and_then(Value::as_str)
                .is_none_or(|value| !valid_identifier(value))
        {
            return Err(DomainWorkflowReconciliationRegistryError::InvalidRecord(
                "provided decision readiness must carry valid subject, state, and audit digest"
                    .into(),
            ));
        }
    } else if audit_digest != Some(&Value::Null)
        || decision_state != Some(&Value::Null)
        || subject_id != Some(&Value::Null)
        || policy_satisfied
    {
        return Err(DomainWorkflowReconciliationRegistryError::InvalidRecord(
            "absent decision readiness must not carry an audit, state, subject, or satisfied policy"
                .into(),
        ));
    }
    Ok(())
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
    if object.get("schema").and_then(Value::as_str)
        != Some("bioprism-devplat-domain-workflow-reconcile/0.1")
    {
        return Err(DomainWorkflowReconciliationRegistryError::InvalidRecord(
            "schema must be domain_workflow_reconcile/0.1".into(),
        ));
    }
    if object.get("execution").and_then(Value::as_str) != Some("not_started") {
        return Err(DomainWorkflowReconciliationRegistryError::InvalidRecord(
            "execution must remain not_started".into(),
        ));
    }
    verify_decision_readiness(object)?;
    let completion = object
        .get("completion")
        .and_then(Value::as_object)
        .ok_or_else(|| {
            DomainWorkflowReconciliationRegistryError::InvalidRecord(
                "completion must be an object".into(),
            )
        })?;
    let completion_status = completion.get("status").and_then(Value::as_str);
    let completion_ready = completion.get("ready").and_then(Value::as_bool);
    let review_required = completion.get("review_required").and_then(Value::as_bool);
    if completion_status.is_none() || completion_ready.is_none() || review_required.is_none() {
        return Err(DomainWorkflowReconciliationRegistryError::InvalidRecord(
            "completion status, ready, and review_required are required".into(),
        ));
    }
    if !matches!(
        completion_status,
        Some("complete" | "complete_with_output_omissions" | "partial" | "failed" | "unverified")
    ) {
        return Err(DomainWorkflowReconciliationRegistryError::InvalidRecord(
            "completion.status is not a recognized reconciliation status".into(),
        ));
    }
    if review_required != Some(true) {
        return Err(DomainWorkflowReconciliationRegistryError::InvalidRecord(
            "completion.review_required must remain true".into(),
        ));
    }
    let evidence = object
        .get("evidence")
        .and_then(Value::as_object)
        .ok_or_else(|| {
            DomainWorkflowReconciliationRegistryError::InvalidRecord(
                "evidence must be an object".into(),
            )
        })?;
    let evidence_valid = evidence.get("evidence_valid").and_then(Value::as_bool);
    if evidence_valid.is_none() {
        return Err(DomainWorkflowReconciliationRegistryError::InvalidRecord(
            "evidence.evidence_valid is required".into(),
        ));
    }
    if completion_ready != evidence_valid {
        return Err(DomainWorkflowReconciliationRegistryError::InvalidRecord(
            "completion.ready must equal evidence.evidence_valid".into(),
        ));
    }
    if (completion_status == Some("complete")) != (evidence_valid == Some(true)) {
        return Err(DomainWorkflowReconciliationRegistryError::InvalidRecord(
            "complete status and evidence_valid must agree".into(),
        ));
    }
    let integrity = object
        .get("integrity")
        .and_then(Value::as_object)
        .ok_or_else(|| {
            DomainWorkflowReconciliationRegistryError::InvalidRecord(
                "integrity must be an object".into(),
            )
        })?;
    let integrity_valid = integrity.get("valid").and_then(Value::as_bool);
    let findings = integrity.get("findings").and_then(Value::as_array);
    let finding_count = integrity.get("finding_count").and_then(Value::as_u64);
    if integrity_valid.is_none() || findings.is_none() || finding_count.is_none() {
        return Err(DomainWorkflowReconciliationRegistryError::InvalidRecord(
            "integrity.valid, integrity.finding_count, and integrity.findings are required".into(),
        ));
    }
    let findings = findings.ok_or_else(|| {
        DomainWorkflowReconciliationRegistryError::InvalidRecord(
            "integrity.findings disappeared during validation".into(),
        )
    })?;
    if findings.len() > MAX_FINDINGS {
        return Err(DomainWorkflowReconciliationRegistryError::InvalidRecord(
            format!("integrity.findings exceeds the {MAX_FINDINGS}-item bound"),
        ));
    }
    if finding_count != Some(findings.len() as u64) {
        return Err(DomainWorkflowReconciliationRegistryError::InvalidRecord(
            "integrity.finding_count does not match integrity.findings".into(),
        ));
    }
    let computed_integrity_valid = findings.iter().try_fold(true, |valid, finding| {
        let finding_object = finding.as_object().ok_or_else(|| {
            DomainWorkflowReconciliationRegistryError::InvalidRecord(
                "integrity.findings entries must be objects".into(),
            )
        })?;
        if finding_object
            .get("code")
            .and_then(Value::as_str)
            .is_none_or(|value| !valid_text(value))
        {
            return Err(DomainWorkflowReconciliationRegistryError::InvalidRecord(
                "integrity finding code must be bounded visible text".into(),
            ));
        }
        let severity = finding_object.get("severity").and_then(Value::as_str);
        if !matches!(severity, Some("error" | "warning")) {
            return Err(DomainWorkflowReconciliationRegistryError::InvalidRecord(
                "integrity finding severity must be error or warning".into(),
            ));
        }
        if finding_object
            .get("message")
            .and_then(Value::as_str)
            .is_none_or(|value| !valid_text(value))
        {
            return Err(DomainWorkflowReconciliationRegistryError::InvalidRecord(
                "integrity finding message must be bounded visible text".into(),
            ));
        }
        if let Some(step_id) = finding_object.get("step_id") {
            if !step_id.is_null()
                && step_id
                    .as_str()
                    .is_none_or(|value| !valid_identifier(value))
            {
                return Err(DomainWorkflowReconciliationRegistryError::InvalidRecord(
                    "integrity finding step_id must be null or a bounded visible identifier".into(),
                ));
            }
        }
        Ok(valid && severity != Some("error"))
    })?;
    if integrity_valid != Some(computed_integrity_valid) {
        return Err(DomainWorkflowReconciliationRegistryError::InvalidRecord(
            "integrity.valid does not match finding severities".into(),
        ));
    }
    for field in ["workflow_id", "mission_id"] {
        if object
            .get(field)
            .and_then(Value::as_str)
            .is_none_or(|value| !valid_identifier(value))
        {
            return Err(DomainWorkflowReconciliationRegistryError::InvalidRecord(
                format!("{field} must be a bounded visible identifier"),
            ));
        }
    }
    if object
        .get("source")
        .and_then(Value::as_str)
        .is_none_or(|value| !valid_text(value))
    {
        return Err(DomainWorkflowReconciliationRegistryError::InvalidRecord(
            "source must be bounded visible text".into(),
        ));
    }
    for field in [
        "workflow_digest",
        "catalog_digest",
        "domain_contract_digest",
        "mission_plan_digest",
        "reconciliation_digest",
    ] {
        if object
            .get(field)
            .and_then(Value::as_str)
            .is_none_or(|value| !valid_digest(value))
        {
            return Err(DomainWorkflowReconciliationRegistryError::InvalidRecord(
                format!("{field} must be a canonical lowercase content hash"),
            ));
        }
    }
    let claimed = object
        .get("reconciliation_digest")
        .and_then(Value::as_str)
        .ok_or_else(|| {
            DomainWorkflowReconciliationRegistryError::InvalidRecord(
                "reconciliation_digest disappeared during validation".into(),
            )
        })?;
    ContentHash::parse(claimed.to_string()).map_err(|_| {
        DomainWorkflowReconciliationRegistryError::InvalidRecord(
            "reconciliation_digest must be a lowercase SHA-256 content hash".into(),
        )
    })?;
    let mut unsigned = record.clone();
    unsigned
        .as_object_mut()
        .ok_or_else(|| {
            DomainWorkflowReconciliationRegistryError::InvalidRecord(
                "record must remain an object while removing reconciliation_digest".into(),
            )
        })?
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

fn valid_text(value: &str) -> bool {
    let trimmed = value.trim();
    !trimmed.is_empty()
        && value == trimmed
        && value.len() <= MAX_TEXT_BYTES
        && !value.chars().any(char::is_control)
}

fn valid_identifier(value: &str) -> bool {
    valid_text(value) && value == value.trim()
}

fn valid_digest(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        && ContentHash::parse(value.to_owned()).is_ok()
}

fn normalized_record(record: &Value) -> Result<Value, DomainWorkflowReconciliationRegistryError> {
    if !record.is_object() {
        return Err(DomainWorkflowReconciliationRegistryError::NotObject);
    }
    let mut normalized = record.clone();
    let object = normalized
        .as_object_mut()
        .ok_or(DomainWorkflowReconciliationRegistryError::NotObject)?;
    object.remove("request_id");
    object.remove("__isError");
    // The MCP/API projection is appended after the canonical reconciliation digest is created.
    // Ignore it on re-import so a caller can pass the exact returned report back to this registry
    // without changing the semantic record identity.
    object.remove("artifact_registry");
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
        "decision_readiness_state": record.pointer("/decision_readiness/decision_state").cloned().unwrap_or(Value::Null),
        "decision_readiness_policy_satisfied": record.pointer("/decision_readiness/policy_satisfied").cloned().unwrap_or(Value::Null),
        "decision_readiness_gate_satisfied": record.get("decision_review_gate_satisfied").cloned().unwrap_or(Value::Null),
        "decision_readiness_audit_digest": record.pointer("/decision_readiness/audit_digest").cloned().unwrap_or(Value::Null),
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
            "integrity": {"valid": true, "finding_count": 0, "findings": []},
            "execution": "not_started"
        });
        let digest = ContentHash::of_value(&record).unwrap().to_string();
        record["reconciliation_digest"] = Value::String(digest);
        record
    }

    fn reseal(record: &mut Value) {
        record
            .as_object_mut()
            .expect("record is an object")
            .remove("reconciliation_digest");
        let digest = ContentHash::of_value(record).unwrap().to_string();
        record["reconciliation_digest"] = json!(digest);
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
                None,
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
    fn snapshot_restore_rejects_contract_drift_and_generation_regression() {
        let mut registry = DomainWorkflowReconciliationRegistry::new();
        registry
            .import(&record("mission-contract", "workspace", "complete"))
            .unwrap();
        let snapshot = registry.snapshot().unwrap();

        let mut retention_drift = snapshot.clone();
        retention_drift["retention"]["max_bytes"] = json!(1);
        retention_drift
            .as_object_mut()
            .unwrap()
            .remove("state_digest");
        retention_drift["state_digest"] = json!(snapshot_digest(&retention_drift).unwrap());
        let error = DomainWorkflowReconciliationRegistry::from_snapshot(&retention_drift)
            .expect_err("retention drift must be refused");
        assert!(error.to_string().contains("retention contract"));

        let mut generation_regression = snapshot;
        generation_regression["generation"] = json!(0);
        generation_regression
            .as_object_mut()
            .unwrap()
            .remove("state_digest");
        generation_regression["state_digest"] =
            json!(snapshot_digest(&generation_regression).unwrap());
        let error = DomainWorkflowReconciliationRegistry::from_snapshot(&generation_regression)
            .expect_err("generation regression must be refused");
        assert!(error.to_string().contains("generation cannot be below"));
    }

    #[test]
    fn import_rejects_a_digest_valid_but_structurally_incomplete_record() {
        let mut invalid = record("mission-incomplete", "workspace", "complete");
        invalid
            .as_object_mut()
            .expect("record is an object")
            .remove("integrity");
        let mut unsigned = invalid.clone();
        unsigned
            .as_object_mut()
            .expect("record is an object")
            .remove("reconciliation_digest");
        let digest = ContentHash::of_value(&unsigned).unwrap().to_string();
        invalid["reconciliation_digest"] = json!(digest);

        let mut registry = DomainWorkflowReconciliationRegistry::new();
        assert!(matches!(
            registry.import(&invalid),
            Err(DomainWorkflowReconciliationRegistryError::InvalidRecord(_))
        ));
    }

    #[test]
    fn import_rejects_noncanonical_embedded_digests_and_invalid_statuses() {
        let mut invalid = record("mission-invalid", "workspace", "complete");
        invalid["workflow_digest"] = json!("A".repeat(64));
        let mut unsigned = invalid.clone();
        unsigned
            .as_object_mut()
            .expect("record is an object")
            .remove("reconciliation_digest");
        invalid["reconciliation_digest"] =
            json!(ContentHash::of_value(&unsigned).unwrap().to_string());

        let mut registry = DomainWorkflowReconciliationRegistry::new();
        let error = registry
            .import(&invalid)
            .expect_err("uppercase digest must be refused");
        assert!(error.to_string().contains("workflow_digest"));

        let mut invalid_status = record("mission-invalid-status", "workspace", "unknown");
        let mut unsigned = invalid_status.clone();
        unsigned
            .as_object_mut()
            .expect("record is an object")
            .remove("reconciliation_digest");
        invalid_status["reconciliation_digest"] =
            json!(ContentHash::of_value(&unsigned).unwrap().to_string());
        let error = registry
            .import(&invalid_status)
            .expect_err("unknown status must be refused");
        assert!(error.to_string().contains("completion.status"));
    }

    #[test]
    fn import_rejects_padded_text_and_inconsistent_integrity_posture() {
        let mut padded = record("mission-padded", "workspace", "complete");
        padded["source"] = json!(" mission_report");
        reseal(&mut padded);
        let mut registry = DomainWorkflowReconciliationRegistry::new();
        let error = registry
            .import(&padded)
            .expect_err("padded source must be refused");
        assert!(error.to_string().contains("source"));

        let mut inconsistent_completion = record("mission-ready-mismatch", "workspace", "complete");
        inconsistent_completion["completion"]["ready"] = json!(false);
        reseal(&mut inconsistent_completion);
        let error = registry
            .import(&inconsistent_completion)
            .expect_err("ready/evidence mismatch must be refused");
        assert!(error.to_string().contains("completion.ready"));

        let mut inconsistent_integrity = record("mission-finding-mismatch", "workspace", "partial");
        inconsistent_integrity["integrity"]["finding_count"] = json!(1);
        inconsistent_integrity["integrity"]["findings"] = json!([{
            "code": "evidence_missing",
            "severity": "error",
            "message": "required evidence is missing",
            "step_id": null
        }]);
        reseal(&mut inconsistent_integrity);
        let error = registry
            .import(&inconsistent_integrity)
            .expect_err("integrity validity mismatch must be refused");
        assert!(error.to_string().contains("integrity.valid"));
    }

    #[test]
    fn import_rejects_contradictory_decision_readiness_projection() {
        let mut invalid = record("mission-readiness-mismatch", "workspace", "partial");
        invalid["decision_readiness"] = json!({
            "required": false,
            "provided": false,
            "subject_id": null,
            "audit_digest": null,
            "decision_state": null,
            "policy_satisfied": false,
            "gate_satisfied": true,
            "readiness_claimed": false,
            "execution": "not_started"
        });
        invalid["decision_review_gate_satisfied"] = json!(false);
        reseal(&mut invalid);
        let mut registry = DomainWorkflowReconciliationRegistry::new();
        let error = registry
            .import(&invalid)
            .expect_err("contradictory decision-readiness posture must be refused");
        assert!(error.to_string().contains("decision readiness gate"));
    }

    #[test]
    fn query_and_get_reject_noncanonical_digest_inputs() {
        let mut registry = DomainWorkflowReconciliationRegistry::new();
        let stored = record("mission-one", "workspace", "complete");
        let digest = stored["reconciliation_digest"].as_str().unwrap().to_owned();
        registry.import(&stored).unwrap();
        assert!(registry.get(&digest.to_uppercase()).is_none());
        assert!(matches!(
            registry.query(
                None,
                None,
                None,
                None,
                None,
                None,
                Some(&digest.to_uppercase()),
                10,
                false,
            ),
            Err(DomainWorkflowReconciliationRegistryError::InvalidSnapshot(
                _
            ))
        ));
    }

    #[test]
    fn import_rejects_generation_counter_overflow_without_mutating_registry() {
        let mut registry = DomainWorkflowReconciliationRegistry {
            generation: u64::MAX,
            records: BTreeMap::new(),
        };
        let error = registry
            .import(&record("mission-overflow", "workspace", "complete"))
            .expect_err("generation overflow must be refused");
        assert_eq!(
            error,
            DomainWorkflowReconciliationRegistryError::GenerationExhausted
        );
        assert_eq!(registry.generation(), u64::MAX);
        assert!(registry.is_empty());
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
            .query(None, None, None, None, None, None, None, 1, false)
            .unwrap();
        assert_eq!(all["rows"].as_array().unwrap().len(), 1);
        assert_eq!(all["has_more"], true);
        let cursor = all["next_after"].as_str().unwrap();
        let next = registry
            .query(None, None, None, None, None, None, Some(cursor), 10, false)
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
        assert_eq!(registry.workflow_posture("oncology")["state"], "incomplete");
    }
}
