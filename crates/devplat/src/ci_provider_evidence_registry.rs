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
use std::collections::BTreeMap;
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
        if ContentHash::parse(digest.to_owned()).is_err() {
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
                ContentHash::parse(value.to_owned()).map_err(|_| {
                    CiProviderEvidenceRegistryError::InvalidSnapshot(format!(
                        "{field} must be a lowercase SHA-256 digest"
                    ))
                })?;
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
        let mut unsigned = document.clone();
        unsigned
            .as_object_mut()
            .expect("snapshot object was checked above")
            .remove("state_digest");
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
        let mut registry = Self {
            generation,
            records: BTreeMap::new(),
        };
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
            let audit: CiProviderEvidenceAudit =
                serde_json::from_value(row_object.get("audit").cloned().ok_or_else(|| {
                    CiProviderEvidenceRegistryError::InvalidSnapshot("audit is missing".into())
                })?)
                .map_err(|error| {
                    CiProviderEvidenceRegistryError::InvalidSnapshot(format!(
                        "record {claimed_digest} is not a typed provider evidence audit: {error}"
                    ))
                })?;
            validate_record(&audit)?;
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
    let mut row = json!({
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
        "finding_count": audit.findings.len(),
        "blocking_finding_count": blocking_findings,
        "artifact_record_digest": audit.artifact_record_digest,
        "log_record_digest": audit.log_record_digest,
        "attestation_record_digest": audit.attestation_record_digest
    });
    Ok(row
        .as_object_mut()
        .expect("index row literal is an object")
        .clone())
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
    if audit.run_id.trim().is_empty() {
        return Err(CiProviderEvidenceRegistryError::InvalidRecord(
            "run_id is empty".into(),
        ));
    }
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
        if ContentHash::parse(value.to_owned()).is_err() {
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
    Ok(())
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
                10,
                false,
            )
            .unwrap();
        assert_eq!(query["rows"].as_array().unwrap().len(), 1);
        let digest = first["provider_evidence_digest"].as_str().unwrap();
        assert_eq!(registry.get(digest).unwrap()["provider"], "github_actions");
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
