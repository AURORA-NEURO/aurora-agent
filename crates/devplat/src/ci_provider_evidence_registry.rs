//! Durable, digest-addressed retention for provider-shaped CI evidence.
//!
//! `ci_provider_evidence_audit` is intentionally a pure review operation: it normalizes a
//! caller-supplied provider payload and checks the exact local CI plan, but its result disappears
//! after the response. This registry supplies the missing operational handoff. Every import
//! re-runs that canonical audit before a record is retained, failed and unknown runs remain
//! queryable as evidence, and restart restores re-check the record identity and outer snapshot
//! digest.
//!
//! This is not a provider connector, signature verifier, log downloader, runner, or release
//! authority. A `conformance_ready` record is a bounded structural handoff signal only.

use crate::ci_provider_evidence::{
    audit_ci_provider_evidence, CiProviderEvidenceAudit, CiProviderEvidenceRequest,
};
use bioprism_ids::ContentHash;
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const CI_PROVIDER_EVIDENCE_REGISTRY_SCHEMA_VERSION: &str =
    "bioprism-devplat-ci-provider-evidence-registry/0.1";
pub const CI_PROVIDER_EVIDENCE_IMPORT_SCHEMA_VERSION: &str =
    "bioprism-devplat-ci-provider-evidence-import/0.1";
pub const CI_PROVIDER_EVIDENCE_QUERY_SCHEMA_VERSION: &str =
    "bioprism-devplat-ci-provider-evidence-query/0.1";
pub const CI_PROVIDER_EVIDENCE_GET_SCHEMA_VERSION: &str =
    "bioprism-devplat-ci-provider-evidence-get/0.1";
pub const MAX_CI_PROVIDER_EVIDENCE_RECORDS: usize = 512;
pub const MAX_CI_PROVIDER_EVIDENCE_REGISTRY_BYTES: usize = 32 * 1024 * 1024;
pub const MAX_CI_PROVIDER_EVIDENCE_QUERY_ITEMS: usize = 256;
const MAX_TEXT_BYTES: usize = 512;
const MAX_FINDINGS: usize = 512;
const MAX_CHECKS: usize = 64;

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum CiProviderEvidenceRegistryError {
    #[error("CI provider evidence input must be an object")]
    NotObject,
    #[error("CI provider evidence request is invalid: {0}")]
    InvalidRequest(String),
    #[error("CI provider evidence audit is invalid: {0}")]
    InvalidRecord(String),
    #[error("CI provider evidence registry has reached its {maximum}-record limit")]
    Full { maximum: usize },
    #[error("CI provider evidence record {digest} already exists with different contents")]
    Conflict { digest: String },
    #[error("CI provider evidence record {digest} was not found")]
    NotFound { digest: String },
    #[error("CI provider evidence snapshot is invalid: {0}")]
    InvalidSnapshot(String),
    #[error("CI provider evidence snapshot is {actual} bytes, above the {maximum}-byte bound")]
    SnapshotTooLarge { actual: usize, maximum: usize },
    #[error("CI provider evidence JSON could not be canonicalized: {0}")]
    Canonical(String),
}

#[derive(Debug, Clone, Default)]
pub struct CiProviderEvidenceRegistry {
    generation: u64,
    records: BTreeMap<String, CiProviderEvidenceAudit>,
}

impl CiProviderEvidenceRegistry {
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

    pub fn digests_for_audit(&self) -> Vec<String> {
        self.records.keys().cloned().collect()
    }

    /// Re-run the canonical provider audit and retain its complete report.
    pub fn import(&mut self, value: &Value) -> Result<Value, CiProviderEvidenceRegistryError> {
        let request: CiProviderEvidenceRequest = serde_json::from_value(value.clone())
            .map_err(|error| CiProviderEvidenceRegistryError::InvalidRequest(error.to_string()))?;
        let audit = audit_ci_provider_evidence(&request)
            .map_err(|error| CiProviderEvidenceRegistryError::InvalidRequest(error.to_string()))?;
        let digest = record_digest(&audit)?;
        if let Some(existing) = self.records.get(&digest) {
            if existing != &audit {
                return Err(CiProviderEvidenceRegistryError::Conflict { digest });
            }
            return Ok(import_response(&audit, &digest, false, true, self));
        }
        if self.records.len() >= MAX_CI_PROVIDER_EVIDENCE_RECORDS {
            return Err(CiProviderEvidenceRegistryError::Full {
                maximum: MAX_CI_PROVIDER_EVIDENCE_RECORDS,
            });
        }
        let mut candidate = self.clone();
        candidate.records.insert(digest.clone(), audit.clone());
        candidate.generation = candidate.generation.saturating_add(1);
        candidate.ensure_snapshot_bound()?;
        self.records = candidate.records;
        self.generation = candidate.generation;
        Ok(import_response(&audit, &digest, true, false, self))
    }

    pub fn get(&self, digest: &str) -> Result<Value, CiProviderEvidenceRegistryError> {
        if !valid_digest(digest) {
            return Err(CiProviderEvidenceRegistryError::InvalidSnapshot(
                "provider_evidence_digest must be a lowercase SHA-256 digest".into(),
            ));
        }
        let audit =
            self.records
                .get(digest)
                .ok_or_else(|| CiProviderEvidenceRegistryError::NotFound {
                    digest: digest.to_owned(),
                })?;
        Ok(json!({
            "ok": true,
            "schema": CI_PROVIDER_EVIDENCE_GET_SCHEMA_VERSION,
            "workflow": "ci_provider_evidence_get",
            "provider_evidence_digest": digest,
            "provider": audit.provider,
            "run_id": audit.run_id,
            "payload_digest": audit.payload_digest,
            "plan_digest": audit.plan_digest,
            "evidence_digest": audit.evidence_digest,
            "local_byte_hash_artifact_count": audit.local_byte_hash_artifact_count,
            "local_byte_hash_log_count": audit.local_byte_hash_log_count,
            "attestation_subject_digest_binding_count": audit.attestation_subject_digest_binding_count,
            "structurally_valid": audit.structurally_valid,
            "conformance_ready": audit.conformance_ready,
            "audit": audit,
            "registry_generation": self.generation,
            "registry_size": self.records.len(),
            "execution": "not_started",
            "guarantees": [
                "the returned record was re-audited before it entered the registry",
                "the lookup identifies one exact canonical provider evidence report"
            ],
            "limitations": [
                "the registry does not prove provider authentication, remote bytes, signatures, or execution"
            ]
        }))
    }

    /// Query compact provider/run/plan posture. Full audits are opt-in.
    #[allow(clippy::too_many_arguments)]
    pub fn query(
        &self,
        provider: Option<&str>,
        run_id: Option<&str>,
        plan_digest: Option<&str>,
        structurally_valid: Option<bool>,
        conformance_ready: Option<bool>,
        min_local_byte_hash_artifacts: Option<usize>,
        min_local_byte_hash_logs: Option<usize>,
        min_attestation_subject_digest_bindings: Option<usize>,
        after: Option<&str>,
        max_items: usize,
        include_records: bool,
    ) -> Result<Value, CiProviderEvidenceRegistryError> {
        if !(1..=MAX_CI_PROVIDER_EVIDENCE_QUERY_ITEMS).contains(&max_items) {
            return Err(CiProviderEvidenceRegistryError::InvalidSnapshot(format!(
                "max_items must be between 1 and {MAX_CI_PROVIDER_EVIDENCE_QUERY_ITEMS}"
            )));
        }
        for (field, value) in [("plan_digest", plan_digest), ("after", after)] {
            if let Some(value) = value {
                if !valid_digest(value) {
                    return Err(CiProviderEvidenceRegistryError::InvalidSnapshot(format!(
                        "{field} must be a lowercase SHA-256 digest"
                    )));
                }
            }
        }
        for (field, value) in [("provider", provider), ("run_id", run_id)] {
            if let Some(value) = value {
                if !valid_text(value) {
                    return Err(CiProviderEvidenceRegistryError::InvalidSnapshot(format!(
                        "{field} must be bounded visible text"
                    )));
                }
            }
        }
        let mut rows = Vec::new();
        let mut has_more = false;
        for (digest, audit) in self
            .records
            .iter()
            .filter(|(digest, _)| after.is_none_or(|cursor| digest.as_str() > cursor))
        {
            if provider.is_some_and(|value| audit.provider != value)
                || run_id.is_some_and(|value| audit.run_id != value)
                || plan_digest.is_some_and(|value| audit.plan_digest != value)
                || structurally_valid.is_some_and(|value| audit.structurally_valid != value)
                || conformance_ready.is_some_and(|value| audit.conformance_ready != value)
                || min_local_byte_hash_artifacts
                    .is_some_and(|value| audit.local_byte_hash_artifact_count < value)
                || min_local_byte_hash_logs
                    .is_some_and(|value| audit.local_byte_hash_log_count < value)
                || min_attestation_subject_digest_bindings
                    .is_some_and(|value| audit.attestation_subject_digest_binding_count < value)
            {
                continue;
            }
            if rows.len() >= max_items {
                has_more = true;
                break;
            }
            let mut row = index_row(digest, audit)?;
            if include_records {
                row.insert(
                    "audit".into(),
                    serde_json::to_value(audit).map_err(|error| {
                        CiProviderEvidenceRegistryError::Canonical(error.to_string())
                    })?,
                );
            }
            rows.push(row);
        }
        let next_after = if has_more {
            rows.last()
                .and_then(|row| row.get("provider_evidence_digest"))
                .cloned()
                .unwrap_or(Value::Null)
        } else {
            Value::Null
        };
        Ok(json!({
            "ok": true,
            "schema": CI_PROVIDER_EVIDENCE_QUERY_SCHEMA_VERSION,
            "workflow": "ci_provider_evidence_query",
            "filters": {
                "provider": provider,
                "run_id": run_id,
                "plan_digest": plan_digest,
                "structurally_valid": structurally_valid,
                "conformance_ready": conformance_ready,
                "min_local_byte_hash_artifacts": min_local_byte_hash_artifacts,
                "min_local_byte_hash_logs": min_local_byte_hash_logs,
                "min_attestation_subject_digest_bindings": min_attestation_subject_digest_bindings,
                "after": after,
                "max_items": max_items,
                "include_records": include_records
            },
            "rows": rows,
            "next_after": next_after,
            "has_more": has_more,
            "registry_generation": self.generation,
            "registry_size": self.records.len(),
            "execution": "not_started",
            "guarantees": [
                "rows are ordered by canonical provider evidence digest",
                "failed, unknown, and incomplete runs remain visible rather than being discarded",
                "minimum digest-binding filters are evaluated against retained audit counts rather than inferred from provider labels",
                "query never contacts a provider or executes a check"
            ],
            "limitations": [
                "results cover only this bounded local registry",
                "structural validity is not provider authentication or release approval"
            ]
        }))
    }

    pub fn snapshot(&self) -> Result<Value, CiProviderEvidenceRegistryError> {
        let mut document = json!({
            "schema": CI_PROVIDER_EVIDENCE_REGISTRY_SCHEMA_VERSION,
            "generation": self.generation,
            "record_count": self.records.len(),
            "records": self.records.iter().map(|(digest, audit)| json!({
                "provider_evidence_digest": digest,
                "audit": audit
            })).collect::<Vec<_>>(),
            "retention": {
                "max_records": MAX_CI_PROVIDER_EVIDENCE_RECORDS,
                "max_bytes": MAX_CI_PROVIDER_EVIDENCE_REGISTRY_BYTES
            },
            "execution": "not_started"
        });
        document["state_digest"] = Value::String(snapshot_digest(&document)?);
        self.ensure_encoded_bound(&document)?;
        Ok(document)
    }

    pub fn from_snapshot(document: &Value) -> Result<Self, CiProviderEvidenceRegistryError> {
        let object = document.as_object().ok_or_else(|| {
            CiProviderEvidenceRegistryError::InvalidSnapshot("snapshot must be an object".into())
        })?;
        let encoded = serde_json::to_vec(document)
            .map_err(|error| CiProviderEvidenceRegistryError::Canonical(error.to_string()))?;
        if encoded.len() > MAX_CI_PROVIDER_EVIDENCE_REGISTRY_BYTES {
            return Err(CiProviderEvidenceRegistryError::SnapshotTooLarge {
                actual: encoded.len(),
                maximum: MAX_CI_PROVIDER_EVIDENCE_REGISTRY_BYTES,
            });
        }
        if object.get("schema").and_then(Value::as_str)
            != Some(CI_PROVIDER_EVIDENCE_REGISTRY_SCHEMA_VERSION)
        {
            return Err(CiProviderEvidenceRegistryError::InvalidSnapshot(
                "schema is invalid".into(),
            ));
        }
        let claimed_state_digest = object
            .get("state_digest")
            .and_then(Value::as_str)
            .ok_or_else(|| {
                CiProviderEvidenceRegistryError::InvalidSnapshot("state_digest is missing".into())
            })?;
        if !valid_digest(claimed_state_digest) {
            return Err(CiProviderEvidenceRegistryError::InvalidSnapshot(
                "state_digest must be a canonical lowercase content hash".into(),
            ));
        }
        let mut unsigned = document.clone();
        let Some(unsigned_object) = unsigned.as_object_mut() else {
            return Err(CiProviderEvidenceRegistryError::InvalidSnapshot(
                "snapshot is not an object after cloning".into(),
            ));
        };
        unsigned_object.remove("state_digest");
        if claimed_state_digest != snapshot_digest(&unsigned)? {
            return Err(CiProviderEvidenceRegistryError::InvalidSnapshot(
                "state_digest does not match snapshot contents".into(),
            ));
        }
        let generation = object
            .get("generation")
            .and_then(Value::as_u64)
            .ok_or_else(|| {
                CiProviderEvidenceRegistryError::InvalidSnapshot("generation is invalid".into())
            })?;
        if object.get("execution").and_then(Value::as_str) != Some("not_started") {
            return Err(CiProviderEvidenceRegistryError::InvalidSnapshot(
                "execution must remain not_started".into(),
            ));
        }
        let retention = object
            .get("retention")
            .and_then(Value::as_object)
            .ok_or_else(|| {
                CiProviderEvidenceRegistryError::InvalidSnapshot(
                    "retention must be an object".into(),
                )
            })?;
        if retention.get("max_records").and_then(Value::as_u64)
            != Some(MAX_CI_PROVIDER_EVIDENCE_RECORDS as u64)
            || retention.get("max_bytes").and_then(Value::as_u64)
                != Some(MAX_CI_PROVIDER_EVIDENCE_REGISTRY_BYTES as u64)
        {
            return Err(CiProviderEvidenceRegistryError::InvalidSnapshot(
                "retention does not match the registry bounds".into(),
            ));
        }
        let rows = object
            .get("records")
            .and_then(Value::as_array)
            .ok_or_else(|| {
                CiProviderEvidenceRegistryError::InvalidSnapshot("records must be an array".into())
            })?;
        if rows.len() > MAX_CI_PROVIDER_EVIDENCE_RECORDS {
            return Err(CiProviderEvidenceRegistryError::Full {
                maximum: MAX_CI_PROVIDER_EVIDENCE_RECORDS,
            });
        }
        if generation < rows.len() as u64 {
            return Err(CiProviderEvidenceRegistryError::InvalidSnapshot(
                "generation cannot be below the retained record count".into(),
            ));
        }
        let mut registry = Self {
            generation,
            records: BTreeMap::new(),
        };
        let mut previous_digest: Option<&str> = None;
        for row in rows {
            let row_object = row.as_object().ok_or_else(|| {
                CiProviderEvidenceRegistryError::InvalidSnapshot(
                    "record row must be an object".into(),
                )
            })?;
            let claimed_digest = row_object
                .get("provider_evidence_digest")
                .and_then(Value::as_str)
                .ok_or_else(|| {
                    CiProviderEvidenceRegistryError::InvalidSnapshot(
                        "provider_evidence_digest is missing".into(),
                    )
                })?;
            if !valid_digest(claimed_digest) {
                return Err(CiProviderEvidenceRegistryError::InvalidSnapshot(
                    "provider_evidence_digest must be a canonical lowercase content hash".into(),
                ));
            }
            if previous_digest.is_some_and(|previous| previous >= claimed_digest) {
                return Err(CiProviderEvidenceRegistryError::InvalidSnapshot(
                    "records must be in strict provider evidence digest order".into(),
                ));
            }
            previous_digest = Some(claimed_digest);
            let audit: CiProviderEvidenceAudit =
                serde_json::from_value(row_object.get("audit").cloned().ok_or_else(|| {
                    CiProviderEvidenceRegistryError::InvalidSnapshot("audit is missing".into())
                })?)
                .map_err(|error| {
                    CiProviderEvidenceRegistryError::InvalidSnapshot(format!(
                        "record {claimed_digest} is not a typed provider evidence audit: {error}"
                    ))
                })?;
            validate_record(&audit).map_err(|error| {
                CiProviderEvidenceRegistryError::InvalidSnapshot(format!(
                    "record {claimed_digest} is invalid: {error}"
                ))
            })?;
            let recomputed = record_digest(&audit)?;
            if recomputed != claimed_digest {
                return Err(CiProviderEvidenceRegistryError::InvalidSnapshot(format!(
                    "record {claimed_digest} failed digest verification"
                )));
            }
            if registry
                .records
                .insert(claimed_digest.to_owned(), audit)
                .is_some()
            {
                return Err(CiProviderEvidenceRegistryError::InvalidSnapshot(
                    "snapshot contains duplicate provider evidence digests".into(),
                ));
            }
        }
        if object.get("record_count").and_then(Value::as_u64) != Some(rows.len() as u64) {
            return Err(CiProviderEvidenceRegistryError::InvalidSnapshot(
                "record_count does not match records".into(),
            ));
        }
        registry.ensure_snapshot_bound()?;
        Ok(registry)
    }

    fn ensure_snapshot_bound(&self) -> Result<(), CiProviderEvidenceRegistryError> {
        let snapshot = self.snapshot()?;
        self.ensure_encoded_bound(&snapshot)
    }

    fn ensure_encoded_bound(
        &self,
        document: &Value,
    ) -> Result<(), CiProviderEvidenceRegistryError> {
        let bytes = serde_json::to_vec(document)
            .map_err(|error| CiProviderEvidenceRegistryError::Canonical(error.to_string()))?;
        if bytes.len() > MAX_CI_PROVIDER_EVIDENCE_REGISTRY_BYTES {
            return Err(CiProviderEvidenceRegistryError::SnapshotTooLarge {
                actual: bytes.len(),
                maximum: MAX_CI_PROVIDER_EVIDENCE_REGISTRY_BYTES,
            });
        }
        Ok(())
    }
}

fn import_response(
    audit: &CiProviderEvidenceAudit,
    digest: &str,
    created: bool,
    already_present: bool,
    registry: &CiProviderEvidenceRegistry,
) -> Value {
    json!({
        "ok": true,
        "schema": CI_PROVIDER_EVIDENCE_IMPORT_SCHEMA_VERSION,
        "workflow": "ci_provider_evidence_import",
        "provider_evidence_digest": digest,
        "provider": audit.provider,
        "run_id": audit.run_id,
        "payload_digest": audit.payload_digest,
        "plan_digest": audit.plan_digest,
        "evidence_digest": audit.evidence_digest,
        "structurally_valid": audit.structurally_valid,
        "conformance_ready": audit.conformance_ready,
        "artifact_count": audit.artifact_count,
        "log_count": audit.log_count,
        "attestation_count": audit.attestation_count,
        "linked_artifact_count": audit.linked_artifact_count,
        "linked_log_count": audit.linked_log_count,
        "attestation_subject_count": audit.attestation_subject_count,
        "local_byte_hash_artifact_count": audit.local_byte_hash_artifact_count,
        "local_byte_hash_log_count": audit.local_byte_hash_log_count,
        "attestation_subject_digest_binding_count": audit.attestation_subject_digest_binding_count,
        "artifact_record_digest": audit.artifact_record_digest,
        "log_record_digest": audit.log_record_digest,
        "attestation_record_digest": audit.attestation_record_digest,
        "created": created,
        "already_present": already_present,
        "registry_generation": registry.generation,
        "registry_size": registry.records.len(),
        "execution": "not_started",
        "guarantees": [
            "the canonical provider audit is rerun before retention",
            "re-importing the same request is idempotent",
            "import does not contact providers, execute checks, or approve a release"
        ],
        "limitations": [
            "the record is a bounded local evidence index rather than a provider archive",
            "provider_observed is a provenance label and not cryptographic provider authentication"
        ]
    })
}

fn index_row(
    digest: &str,
    audit: &CiProviderEvidenceAudit,
) -> Result<serde_json::Map<String, Value>, CiProviderEvidenceRegistryError> {
    let blocking_findings = audit
        .findings
        .iter()
        .filter(|finding| finding.severity == "blocking")
        .count();
    let row = json!({
        "provider_evidence_digest": digest,
        "schema": audit.schema,
        "provider": audit.provider,
        "source": audit.source,
        "run_id": audit.run_id,
        "payload_digest": audit.payload_digest,
        "plan_digest": audit.plan_digest,
        "evidence_digest": audit.evidence_digest,
        "conclusion": audit.evidence.conclusion,
        "structurally_valid": audit.structurally_valid,
        "conformance_ready": audit.conformance_ready,
        "artifact_count": audit.artifact_count,
        "log_count": audit.log_count,
        "attestation_count": audit.attestation_count,
        "linked_artifact_count": audit.linked_artifact_count,
        "linked_log_count": audit.linked_log_count,
        "attestation_subject_count": audit.attestation_subject_count,
        "local_byte_hash_artifact_count": audit.local_byte_hash_artifact_count,
        "local_byte_hash_log_count": audit.local_byte_hash_log_count,
        "attestation_subject_digest_binding_count": audit.attestation_subject_digest_binding_count,
        "finding_count": audit.findings.len(),
        "blocking_finding_count": blocking_findings,
        "artifact_record_digest": audit.artifact_record_digest,
        "log_record_digest": audit.log_record_digest,
        "attestation_record_digest": audit.attestation_record_digest
    });
    let Some(object) = row.as_object() else {
        return Err(CiProviderEvidenceRegistryError::InvalidRecord(
            "index row is not an object".into(),
        ));
    };
    Ok(object.clone())
}

fn validate_finding(
    finding: &crate::ci_evidence::CiEvidenceFinding,
    field: &str,
) -> Result<(), CiProviderEvidenceRegistryError> {
    if !valid_text(&finding.code)
        || finding.severity != "blocking"
        || !valid_text(&finding.subject)
        || !valid_text(&finding.detail)
    {
        return Err(CiProviderEvidenceRegistryError::InvalidRecord(format!(
            "{field} contains a malformed canonical finding"
        )));
    }
    Ok(())
}

fn validate_findings(
    findings: &[crate::ci_evidence::CiEvidenceFinding],
    field: &str,
) -> Result<(), CiProviderEvidenceRegistryError> {
    if findings.len() > MAX_FINDINGS {
        return Err(CiProviderEvidenceRegistryError::InvalidRecord(format!(
            "{field} exceeds the {MAX_FINDINGS}-finding bound"
        )));
    }
    for finding in findings {
        validate_finding(finding, field)?;
    }
    Ok(())
}

fn validate_run_evidence(
    audit: &CiProviderEvidenceAudit,
) -> Result<(), CiProviderEvidenceRegistryError> {
    let evidence = &audit.evidence;
    if !valid_text(&evidence.run_id)
        || !valid_text(&evidence.provider)
        || !valid_digest(&evidence.plan_digest)
        || evidence.checks.is_empty()
        || evidence.checks.len() > MAX_CHECKS
        || evidence
            .environment_digest
            .as_deref()
            .is_some_and(|value| !valid_digest(value))
        || evidence
            .run_url
            .as_deref()
            .is_some_and(|value| !valid_text(value))
    {
        return Err(CiProviderEvidenceRegistryError::InvalidRecord(
            "retained CI run evidence has invalid identity, digest, URL, or check bounds".into(),
        ));
    }
    for check in &evidence.checks {
        if !valid_text(&check.name)
            || !valid_digest(&check.result_digest)
            || check
                .detail
                .as_deref()
                .is_some_and(|value| !valid_text(value))
        {
            return Err(CiProviderEvidenceRegistryError::InvalidRecord(
                "retained CI check evidence has invalid identity, digest, or detail".into(),
            ));
        }
    }
    if evidence.run_id != audit.run_id
        || evidence.provider != audit.provider
        || evidence.source != audit.source
        || evidence.plan_digest != audit.plan_digest
        || evidence.conclusion != audit.ci_evidence.conclusion
    {
        return Err(CiProviderEvidenceRegistryError::InvalidRecord(
            "retained CI run evidence is not bound to the audit identity".into(),
        ));
    }
    let passed = evidence
        .checks
        .iter()
        .filter(|check| check.status == crate::ci_evidence::CiCheckStatus::Passed)
        .count();
    let failed = evidence
        .checks
        .iter()
        .filter(|check| check.status == crate::ci_evidence::CiCheckStatus::Failed)
        .count();
    let skipped = evidence
        .checks
        .iter()
        .filter(|check| check.status == crate::ci_evidence::CiCheckStatus::Skipped)
        .count();
    let unknown = evidence
        .checks
        .iter()
        .filter(|check| {
            matches!(
                check.status,
                crate::ci_evidence::CiCheckStatus::Cancelled
                    | crate::ci_evidence::CiCheckStatus::Unknown
            )
        })
        .count();
    if audit.ci_evidence.observed_check_count > audit.evidence.checks.len()
        || audit.ci_evidence.passed_check_count > passed
        || audit.ci_evidence.failed_check_count > failed
        || audit.ci_evidence.skipped_check_count > skipped
        || audit.ci_evidence.unknown_check_count > unknown
        || audit.ci_evidence.passed_check_count > audit.ci_evidence.observed_check_count
        || audit.ci_evidence.failed_check_count > audit.ci_evidence.observed_check_count
        || audit.ci_evidence.skipped_check_count > audit.ci_evidence.observed_check_count
        || audit.ci_evidence.unknown_check_count > audit.ci_evidence.observed_check_count
        || audit.ci_evidence.passed_check_count
            + audit.ci_evidence.failed_check_count
            + audit.ci_evidence.skipped_check_count
            + audit.ci_evidence.unknown_check_count
            != audit.ci_evidence.observed_check_count
    {
        return Err(CiProviderEvidenceRegistryError::InvalidRecord(
            "CI check status counts are inconsistent".into(),
        ));
    }
    Ok(())
}

fn validate_record(audit: &CiProviderEvidenceAudit) -> Result<(), CiProviderEvidenceRegistryError> {
    if audit.schema != crate::ci_provider_evidence::CI_PROVIDER_EVIDENCE_SCHEMA
        || audit.workflow != "ci_provider_evidence_audit"
    {
        return Err(CiProviderEvidenceRegistryError::InvalidRecord(
            "schema or workflow is invalid".into(),
        ));
    }
    if audit.provider != "github_actions"
        && audit.provider != "gitlab_ci"
        && audit.provider != "generic"
    {
        return Err(CiProviderEvidenceRegistryError::InvalidRecord(
            "provider is unsupported".into(),
        ));
    }
    if !valid_text(&audit.run_id) {
        return Err(CiProviderEvidenceRegistryError::InvalidRecord(
            "run_id is invalid".into(),
        ));
    }
    validate_findings(&audit.findings, "provider findings")?;
    validate_findings(&audit.ci_evidence.findings, "CI evidence findings")?;
    validate_run_evidence(audit)?;
    for (field, value) in [
        ("payload_digest", &audit.payload_digest),
        ("plan_digest", &audit.plan_digest),
        ("evidence_digest", &audit.evidence_digest),
        ("artifact_record_digest", &audit.artifact_record_digest),
        ("log_record_digest", &audit.log_record_digest),
        (
            "attestation_record_digest",
            &audit.attestation_record_digest,
        ),
    ] {
        if !valid_digest(value) {
            return Err(CiProviderEvidenceRegistryError::InvalidRecord(format!(
                "{field} is not a lowercase SHA-256 digest"
            )));
        }
    }
    if audit.artifact_count != audit.artifacts.len()
        || audit.log_count != audit.logs.len()
        || audit.attestation_count != audit.attestations.len()
    {
        return Err(CiProviderEvidenceRegistryError::InvalidRecord(
            "record counts do not match retained rows".into(),
        ));
    }
    let mut subject_keys = BTreeSet::from([audit.run_id.to_ascii_lowercase()]);
    for artifact in &audit.artifacts {
        if !valid_text(&artifact.id)
            || !valid_text(&artifact.kind)
            || !valid_digest(&artifact.digest)
            || !subject_keys.insert(artifact.id.to_ascii_lowercase())
        {
            return Err(CiProviderEvidenceRegistryError::InvalidRecord(
                "artifact identity, digest, or subject namespace is invalid".into(),
            ));
        }
        if artifact.provider.as_deref() != Some(audit.provider.as_str())
            || artifact.run_id.as_deref() != Some(audit.run_id.as_str())
            || artifact
                .check
                .as_deref()
                .is_some_and(|value| !valid_text(value))
            || artifact
                .uri
                .as_deref()
                .is_some_and(|value| !valid_text(value))
        {
            return Err(CiProviderEvidenceRegistryError::InvalidRecord(
                "artifact provider, run, check, or URI binding is invalid".into(),
            ));
        }
        validate_digest_scope(artifact.digest_scope.as_deref(), artifact.uri.as_deref())?;
    }
    for log in &audit.logs {
        if !valid_text(&log.id)
            || !valid_digest(&log.digest)
            || !subject_keys.insert(log.id.to_ascii_lowercase())
        {
            return Err(CiProviderEvidenceRegistryError::InvalidRecord(
                "log identity, digest, or subject namespace is invalid".into(),
            ));
        }
        if log.provider.as_deref() != Some(audit.provider.as_str())
            || log.run_id.as_deref() != Some(audit.run_id.as_str())
            || log.check.as_deref().is_some_and(|value| !valid_text(value))
            || log.uri.as_deref().is_some_and(|value| !valid_text(value))
        {
            return Err(CiProviderEvidenceRegistryError::InvalidRecord(
                "log provider, run, check, or URI binding is invalid".into(),
            ));
        }
        validate_digest_scope(log.digest_scope.as_deref(), log.uri.as_deref())?;
    }
    let mut attestation_ids = BTreeSet::new();
    for attestation in &audit.attestations {
        if !valid_text(&attestation.id)
            || !valid_text(&attestation.subject)
            || !valid_text(&attestation.issuer)
            || !valid_text(&attestation.method)
            || !valid_digest(&attestation.statement_digest)
            || !attestation_ids.insert(attestation.id.to_ascii_lowercase())
            || attestation
                .subject_digest
                .as_deref()
                .is_some_and(|digest| !valid_digest(digest))
        {
            return Err(CiProviderEvidenceRegistryError::InvalidRecord(
                "attestation identity, digest, or subject namespace is invalid".into(),
            ));
        }
    }
    if audit.execution != "evidence_supplied_not_executed_here"
        || audit.verification != "structural_only"
            && audit.verification != "structural_only_with_digest_bindings"
    {
        return Err(CiProviderEvidenceRegistryError::InvalidRecord(
            "record execution or verification posture is invalid".into(),
        ));
    }
    if audit.evidence.provider != audit.provider
        || audit.evidence.source != audit.source
        || audit.evidence.run_id != audit.run_id
        || audit.ci_evidence.schema != crate::ci_evidence::CI_EXECUTION_EVIDENCE_SCHEMA
        || audit.ci_evidence.plan_digest != audit.plan_digest
        || audit.ci_evidence.evidence_digest != audit.evidence_digest
        || audit.ci_evidence.run_id != audit.run_id
        || audit.ci_evidence.provider != audit.provider
        || audit.ci_evidence.source != audit.source
        || audit.ci_evidence.execution != "evidence_supplied_not_executed_here"
        || audit.ci_evidence.verification != "structural_only"
    {
        return Err(CiProviderEvidenceRegistryError::InvalidRecord(
            "nested CI evidence identity is inconsistent".into(),
        ));
    }
    let evidence_value = serde_json::to_value((&audit.ci_evidence.plan_digest, &audit.evidence))
        .map_err(|error| CiProviderEvidenceRegistryError::Canonical(error.to_string()))?;
    let recomputed_evidence_digest = ContentHash::of_value(&evidence_value)
        .map_err(|error| CiProviderEvidenceRegistryError::Canonical(error.to_string()))?
        .to_string();
    if recomputed_evidence_digest != audit.evidence_digest {
        return Err(CiProviderEvidenceRegistryError::InvalidRecord(
            "evidence_digest does not match the retained run evidence".into(),
        ));
    }
    if audit.artifact_record_digest != rows_digest(&audit.artifacts)?
        || audit.log_record_digest != rows_digest(&audit.logs)?
        || audit.attestation_record_digest != rows_digest(&audit.attestations)?
    {
        return Err(CiProviderEvidenceRegistryError::InvalidRecord(
            "retained row digests do not match their rows".into(),
        ));
    }
    let linked_artifacts = audit
        .artifacts
        .iter()
        .filter(|artifact| artifact.check.is_some())
        .count();
    let linked_logs = audit.logs.iter().filter(|log| log.check.is_some()).count();
    let local_artifacts = audit
        .artifacts
        .iter()
        .filter(|artifact| {
            artifact.digest_scope.as_deref()
                == Some(crate::ci_provider_evidence::DIGEST_SCOPE_LOCAL_RESPONSE_BYTES)
        })
        .count();
    let local_logs = audit
        .logs
        .iter()
        .filter(|log| {
            log.digest_scope.as_deref()
                == Some(crate::ci_provider_evidence::DIGEST_SCOPE_LOCAL_RESPONSE_BYTES)
        })
        .count();
    if audit.linked_artifact_count != linked_artifacts
        || audit.linked_log_count != linked_logs
        || audit.local_byte_hash_artifact_count != local_artifacts
        || audit.local_byte_hash_log_count != local_logs
    {
        return Err(CiProviderEvidenceRegistryError::InvalidRecord(
            "derived artifact and log counts are inconsistent".into(),
        ));
    }
    let mut known_subjects = std::collections::BTreeSet::from([audit.run_id.clone()]);
    let mut known_digests = BTreeMap::from([(audit.run_id.clone(), audit.payload_digest.clone())]);
    for artifact in &audit.artifacts {
        known_subjects.insert(artifact.id.clone());
        known_digests.insert(artifact.id.clone(), artifact.digest.clone());
    }
    for log in &audit.logs {
        known_subjects.insert(log.id.clone());
        known_digests.insert(log.id.clone(), log.digest.clone());
    }
    let subject_count = audit
        .attestations
        .iter()
        .filter(|attestation| known_subjects.contains(&attestation.subject))
        .count();
    let subject_digest_binding_count = audit
        .attestations
        .iter()
        .filter(|attestation| {
            attestation
                .subject_digest
                .as_ref()
                .zip(known_digests.get(&attestation.subject))
                .is_some_and(|(digest, expected)| digest == expected)
        })
        .count();
    if audit.attestation_subject_count != subject_count
        || audit.attestation_subject_digest_binding_count != subject_digest_binding_count
    {
        return Err(CiProviderEvidenceRegistryError::InvalidRecord(
            "derived attestation subject counts are inconsistent".into(),
        ));
    }
    let expected_verification =
        if local_artifacts > 0 || local_logs > 0 || subject_digest_binding_count > 0 {
            "structural_only_with_digest_bindings"
        } else {
            "structural_only"
        };
    if audit.verification != expected_verification {
        return Err(CiProviderEvidenceRegistryError::InvalidRecord(
            "verification posture does not match retained digest bindings".into(),
        ));
    }
    let ci_structurally_valid = audit.ci_evidence.findings.iter().all(|finding| {
        !matches!(
            finding.code.as_str(),
            "plan_digest_mismatch"
                | "duplicate_check_evidence"
                | "unknown_check_evidence"
                | "missing_check_evidence"
        )
    });
    let ci_complete = audit.ci_evidence.required_missing.is_empty()
        && audit.ci_evidence.observed_check_count == audit.ci_evidence.expected_check_count;
    let ci_release_candidate = ci_structurally_valid
        && ci_complete
        && audit.ci_evidence.conclusion == crate::ci_evidence::CiRunConclusion::Success
        && audit.ci_evidence.passed_check_count == audit.ci_evidence.expected_check_count;
    let expected_structurally_valid = audit.ci_evidence.structurally_valid
        && audit
            .findings
            .iter()
            .all(|finding| finding.severity != "blocking");
    if audit.ci_evidence.structurally_valid != ci_structurally_valid
        || audit.ci_evidence.complete != ci_complete
        || audit.ci_evidence.release_candidate != ci_release_candidate
        || audit.structurally_valid != expected_structurally_valid
        || audit.conformance_ready
            != (audit.structurally_valid && audit.ci_evidence.release_candidate)
    {
        return Err(CiProviderEvidenceRegistryError::InvalidRecord(
            "nested CI evidence readiness fields are inconsistent".into(),
        ));
    }
    if audit.guarantees.is_empty()
        || audit.limitations.is_empty()
        || audit.guarantees.iter().any(|value| !valid_text(value))
        || audit.limitations.iter().any(|value| !valid_text(value))
    {
        return Err(CiProviderEvidenceRegistryError::InvalidRecord(
            "record guarantees and limitations must be non-empty".into(),
        ));
    }
    Ok(())
}

fn valid_text(value: &str) -> bool {
    !value.trim().is_empty()
        && value.len() <= MAX_TEXT_BYTES
        && value == value.trim()
        && !value.chars().any(char::is_control)
}

fn validate_digest_scope(
    scope: Option<&str>,
    uri: Option<&str>,
) -> Result<(), CiProviderEvidenceRegistryError> {
    match scope {
        None
        | Some(crate::ci_provider_evidence::DIGEST_SCOPE_PROVIDER_METADATA)
        | Some(crate::ci_provider_evidence::DIGEST_SCOPE_CALLER_DECLARED) => Ok(()),
        Some(crate::ci_provider_evidence::DIGEST_SCOPE_LOCAL_RESPONSE_BYTES) if uri.is_some() => {
            Ok(())
        }
        Some(crate::ci_provider_evidence::DIGEST_SCOPE_LOCAL_RESPONSE_BYTES) => {
            Err(CiProviderEvidenceRegistryError::InvalidRecord(
                "local_response_bytes digest scope requires a source URI".into(),
            ))
        }
        Some(_) => Err(CiProviderEvidenceRegistryError::InvalidRecord(
            "digest scope is unsupported".into(),
        )),
    }
}

fn valid_digest(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        && ContentHash::parse(value.to_owned()).is_ok()
}

fn rows_digest<T: serde::Serialize>(rows: &[T]) -> Result<String, CiProviderEvidenceRegistryError> {
    let value = serde_json::to_value(rows)
        .map_err(|error| CiProviderEvidenceRegistryError::Canonical(error.to_string()))?;
    ContentHash::of_value(&value)
        .map(|digest| digest.to_string())
        .map_err(|error| CiProviderEvidenceRegistryError::Canonical(error.to_string()))
}

fn record_digest(
    audit: &CiProviderEvidenceAudit,
) -> Result<String, CiProviderEvidenceRegistryError> {
    validate_record(audit)?;
    let value = serde_json::to_value(audit)
        .map_err(|error| CiProviderEvidenceRegistryError::Canonical(error.to_string()))?;
    ContentHash::of_value(&value)
        .map(|digest| digest.to_string())
        .map_err(|error| CiProviderEvidenceRegistryError::Canonical(error.to_string()))
}

fn snapshot_digest(document: &Value) -> Result<String, CiProviderEvidenceRegistryError> {
    ContentHash::of_value(document)
        .map(|digest| digest.to_string())
        .map_err(|error| CiProviderEvidenceRegistryError::Canonical(error.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ci_evidence::CiEvidenceSource;
    use crate::workbench::{CiCheck, CiRequest};

    fn request(run_id: i64, conclusion: &str) -> Value {
        serde_json::json!({
            "ci": {
                "workflow": "registry-ci",
                "triggers": ["push"],
                "rust_toolchain": "stable",
                "checks": [{"name": "unit", "run": "cargo test -p core", "required": true}],
                "offline": true
            },
            "provider": "github_actions",
            "source": "provider_observed",
            "payload": {
                "run": {"id": run_id, "conclusion": conclusion},
                "jobs": [{"name": "unit", "conclusion": conclusion}]
            }
        })
    }

    #[test]
    fn import_is_audited_idempotent_and_queryable() {
        let mut registry = CiProviderEvidenceRegistry::new();
        let first = registry.import(&request(42, "success")).unwrap();
        let second = registry.import(&request(42, "success")).unwrap();
        assert_eq!(first["created"], true);
        assert_eq!(second["already_present"], true);
        assert_eq!(registry.len(), 1);
        let query = registry
            .query(
                Some("github_actions"),
                Some("42"),
                None,
                Some(true),
                Some(true),
                None,
                None,
                None,
                None,
                10,
                false,
            )
            .unwrap();
        assert_eq!(query["rows"].as_array().unwrap().len(), 1);
        assert_eq!(query["rows"][0]["local_byte_hash_artifact_count"], 0);
        let digest = first["provider_evidence_digest"].as_str().unwrap();
        assert_eq!(registry.get(digest).unwrap()["provider"], "github_actions");
    }

    #[test]
    fn query_can_require_retained_digest_binding_posture() {
        let mut registry = CiProviderEvidenceRegistry::new();
        let mut value = request(45, "success");
        value["artifacts"] = serde_json::json!([{
            "id": "artifact-45",
            "kind": "package",
            "digest": "a".repeat(64),
            "run_id": "45",
            "provider": "github_actions",
            "uri": "https://example.test/artifact-45",
            "digest_scope": "local_response_bytes"
        }]);
        value["attestations"] = serde_json::json!([{
            "id": "attestation-45",
            "subject": "artifact-45",
            "issuer": "caller",
            "statement_digest": "b".repeat(64),
            "method": "declared_provider_statement",
            "subject_digest": "a".repeat(64)
        }]);
        let imported = registry.import(&value).unwrap();
        assert_eq!(imported["local_byte_hash_artifact_count"], 1);
        assert_eq!(imported["attestation_subject_digest_binding_count"], 1);

        let matching = registry
            .query(
                Some("github_actions"),
                Some("45"),
                None,
                Some(true),
                Some(true),
                Some(1),
                None,
                Some(1),
                None,
                10,
                false,
            )
            .unwrap();
        assert_eq!(matching["rows"].as_array().unwrap().len(), 1);

        let absent = registry
            .query(
                Some("github_actions"),
                Some("45"),
                None,
                Some(true),
                Some(true),
                Some(2),
                None,
                None,
                None,
                10,
                false,
            )
            .unwrap();
        assert!(absent["rows"].as_array().unwrap().is_empty());
    }

    #[test]
    fn failed_runs_remain_retained_and_snapshot_tampering_is_rejected() {
        let mut registry = CiProviderEvidenceRegistry::new();
        let imported = registry.import(&request(43, "failure")).unwrap();
        assert_eq!(imported["structurally_valid"], true);
        assert_eq!(imported["conformance_ready"], false);
        let snapshot = registry.snapshot().unwrap();
        let restored = CiProviderEvidenceRegistry::from_snapshot(&snapshot).unwrap();
        assert_eq!(restored.len(), 1);
        let mut tampered = snapshot;
        tampered["records"][0]["audit"]["run_id"] = serde_json::json!("tampered");
        assert!(CiProviderEvidenceRegistry::from_snapshot(&tampered).is_err());
    }

    #[test]
    fn imported_request_recomputes_canonical_provider_evidence() {
        let typed: CiProviderEvidenceRequest =
            serde_json::from_value(request(44, "success")).unwrap();
        assert_eq!(typed.source, Some(CiEvidenceSource::ProviderObserved));
        assert_eq!(typed.ci.checks.len(), 1);
        let mut registry = CiProviderEvidenceRegistry::new();
        let report = registry
            .import(&serde_json::to_value(&typed).unwrap())
            .unwrap();
        assert_eq!(report["plan_digest"].as_str().unwrap().len(), 64);
    }

    fn reseal_snapshot(mut document: Value) -> Value {
        document
            .as_object_mut()
            .expect("snapshot fixture must be an object")
            .remove("state_digest");
        let digest = snapshot_digest(&document).unwrap();
        document["state_digest"] = Value::String(digest);
        document
    }

    #[test]
    fn digest_valid_snapshot_metadata_still_has_to_match_registry_contract() {
        let mut registry = CiProviderEvidenceRegistry::new();
        registry.import(&request(46, "success")).unwrap();

        let mut execution = registry.snapshot().unwrap();
        execution["execution"] = json!("executed");
        assert!(matches!(
            CiProviderEvidenceRegistry::from_snapshot(&reseal_snapshot(execution)),
            Err(CiProviderEvidenceRegistryError::InvalidSnapshot(_))
        ));

        let mut retention = registry.snapshot().unwrap();
        retention["retention"]["max_records"] = json!(1);
        assert!(matches!(
            CiProviderEvidenceRegistry::from_snapshot(&reseal_snapshot(retention)),
            Err(CiProviderEvidenceRegistryError::InvalidSnapshot(_))
        ));

        let mut generation = registry.snapshot().unwrap();
        generation["generation"] = json!(0);
        assert!(matches!(
            CiProviderEvidenceRegistry::from_snapshot(&reseal_snapshot(generation)),
            Err(CiProviderEvidenceRegistryError::InvalidSnapshot(_))
        ));
    }

    #[test]
    fn resealed_snapshot_rejects_noncanonical_record_order() {
        let mut registry = CiProviderEvidenceRegistry::new();
        registry.import(&request(51, "success")).unwrap();
        registry.import(&request(52, "success")).unwrap();
        let mut snapshot = registry.snapshot().unwrap();
        snapshot["records"].as_array_mut().unwrap().reverse();
        assert!(matches!(
            CiProviderEvidenceRegistry::from_snapshot(&reseal_snapshot(snapshot)),
            Err(CiProviderEvidenceRegistryError::InvalidSnapshot(message))
                if message.contains("strict provider evidence digest order")
        ));
    }

    #[test]
    fn digest_valid_record_tampering_cannot_break_nested_identity() {
        let mut registry = CiProviderEvidenceRegistry::new();
        registry.import(&request(47, "success")).unwrap();
        let mut snapshot = registry.snapshot().unwrap();
        snapshot["records"][0]["audit"]["artifact_record_digest"] = json!("a".repeat(64));
        assert!(matches!(
            CiProviderEvidenceRegistry::from_snapshot(&reseal_snapshot(snapshot)),
            Err(CiProviderEvidenceRegistryError::InvalidSnapshot(_))
        ));
    }

    #[test]
    fn query_rejects_an_untyped_cursor() {
        let registry = CiProviderEvidenceRegistry::new();
        assert!(matches!(
            registry.query(
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                Some("not-a-digest"),
                1,
                false
            ),
            Err(CiProviderEvidenceRegistryError::InvalidSnapshot(_))
        ));
        assert!(matches!(
            registry.query(
                Some(" github_actions"),
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                1,
                false
            ),
            Err(CiProviderEvidenceRegistryError::InvalidSnapshot(_))
        ));
    }

    #[test]
    fn registry_rejects_noncanonical_nested_digests_and_control_identity() {
        let mut registry = CiProviderEvidenceRegistry::new();
        registry.import(&request(48, "success")).unwrap();
        let snapshot = registry.snapshot().unwrap();
        let mut audit: CiProviderEvidenceAudit =
            serde_json::from_value(snapshot["records"][0]["audit"].clone()).unwrap();
        let original_payload_digest = audit.payload_digest.clone();
        audit.payload_digest = "A".repeat(64);
        assert!(record_digest(&audit).is_err());
        audit.payload_digest = original_payload_digest;
        audit.run_id = "48\u{0000}".into();
        assert!(record_digest(&audit).is_err());
    }

    #[test]
    fn registry_rejects_padded_bindings_invalid_scopes_and_false_conformance() {
        let mut registry = CiProviderEvidenceRegistry::new();
        let mut value = request(49, "success");
        value["artifacts"] = serde_json::json!([{
            "id": "artifact-49",
            "kind": "package",
            "digest": "a".repeat(64),
            "check": "unit",
            "run_id": "49",
            "provider": "github_actions",
            "uri": "https://example.test/artifact-49"
        }]);
        registry.import(&value).unwrap();
        let snapshot = registry.snapshot().unwrap();
        let mut audit: CiProviderEvidenceAudit =
            serde_json::from_value(snapshot["records"][0]["audit"].clone()).unwrap();

        audit.artifacts[0].run_id = Some(" 49".into());
        assert!(record_digest(&audit).is_err());

        audit.artifacts[0].run_id = Some("49".into());
        audit.artifacts[0].digest_scope = Some("untrusted_remote_claim".into());
        assert!(record_digest(&audit).is_err());

        audit.artifacts[0].digest_scope = None;
        audit.conformance_ready = false;
        assert!(record_digest(&audit).is_err());
    }

    #[test]
    fn registry_rejects_malformed_nested_run_evidence_and_findings() {
        let mut registry = CiProviderEvidenceRegistry::new();
        registry.import(&request(50, "failure")).unwrap();
        let snapshot = registry.snapshot().unwrap();
        let mut audit: CiProviderEvidenceAudit =
            serde_json::from_value(snapshot["records"][0]["audit"].clone()).unwrap();

        audit.evidence.checks[0].detail = Some(" padded detail".into());
        assert!(record_digest(&audit).is_err());

        let mut audit: CiProviderEvidenceAudit =
            serde_json::from_value(snapshot["records"][0]["audit"].clone()).unwrap();
        audit.ci_evidence.findings[0].severity = "warning".into();
        assert!(record_digest(&audit).is_err());
    }

    #[allow(dead_code)]
    fn _typed_request_is_constructible() -> CiProviderEvidenceRequest {
        CiProviderEvidenceRequest {
            ci: CiRequest {
                workflow: "ci".into(),
                triggers: vec!["push".into()],
                rust_toolchain: "stable".into(),
                checks: vec![CiCheck {
                    name: "unit".into(),
                    run: "cargo test".into(),
                    working_directory: None,
                    required: true,
                }],
                offline: true,
            },
            provider: "generic".into(),
            payload: json!({"run_id": "1", "checks": [{"name": "unit", "status": "success"}]}),
            source: Some(CiEvidenceSource::CallerAttested),
            artifacts: Vec::new(),
            logs: Vec::new(),
            attestations: Vec::new(),
        }
    }
}
