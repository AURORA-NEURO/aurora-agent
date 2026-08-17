//! Cross-domain, digest-addressed indexing for artifacts produced by missions and audits.
//!
//! A mission report, evaluator replay, workflow reconciliation, or evidence bundle is useful
//! only when a later caller can answer three separate questions: what bytes were indexed, which
//! integrity check ran, and which declared parent artifacts were visible at registration time.
//! This registry answers those questions without turning an index row into scientific, clinical,
//! provenance, publication, or external-effect authority.
//!
//! The registry is deliberately bounded and deterministic. Records are keyed by the SHA-256 of
//! the exact artifact JSON, while known artifact formats retain their own declared digest in a
//! separate field. That prevents a bundle's internal digest convention from being confused with
//! the digest used by this cross-domain index.

use crate::domain_evidence::{
    validate_domain_evidence_harmonization, DOMAIN_EVIDENCE_HARMONIZATION_SCHEMA_VERSION,
};
use crate::domain_evidence_intake::{
    validate_domain_evidence_intake, DOMAIN_EVIDENCE_INTAKE_SCHEMA_VERSION,
};
use crate::domain_report::{validate_domain_report, DOMAIN_REPORT_SCHEMA_VERSION};
use crate::evidence_bundle::verify_mission_evidence_bundle;
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use thiserror::Error;

pub const ARTIFACT_REGISTRY_SCHEMA_VERSION: &str = "bioprism-devplat-artifact-registry/0.1";
pub const ARTIFACT_REGISTRY_REGISTER_SCHEMA_VERSION: &str =
    "bioprism-devplat-artifact-register/0.1";
pub const ARTIFACT_REGISTRY_QUERY_SCHEMA_VERSION: &str = "bioprism-devplat-artifact-query/0.1";
pub const ARTIFACT_REGISTRY_LINEAGE_SCHEMA_VERSION: &str = "bioprism-devplat-artifact-lineage/0.1";
pub const ARTIFACT_REGISTRY_GET_SCHEMA_VERSION: &str = "bioprism-devplat-artifact-get/0.1";
pub const MAX_ARTIFACT_REGISTRY_RECORDS: usize = 512;
pub const MAX_ARTIFACT_REGISTRY_BYTES: usize = 64 * 1024 * 1024;
pub const MAX_ARTIFACT_REGISTRY_QUERY_ITEMS: usize = 256;
pub const MAX_ARTIFACT_REGISTRY_PARENTS: usize = 128;
pub const MAX_ARTIFACT_REGISTRY_DOMAINS: usize = 128;
pub const MAX_ARTIFACT_REGISTRY_LINEAGE_NODES: usize = 512;
pub const MAX_ARTIFACT_REGISTRY_TEXT_BYTES: usize = 512;

const ARTIFACT_KINDS: &[&str] = &[
    "mission_evidence_bundle",
    "workflow_reconciliation",
    "mission_report",
    "evaluator_replay",
    "domain_report",
    "domain_evidence_harmonization",
    "domain_evidence_intake",
    "external_reference",
];

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ArtifactRegistryError {
    #[error("artifact registration must be an object")]
    RegistrationNotObject,
    #[error("artifact registry input is invalid: {0}")]
    InvalidInput(String),
    #[error("artifact registry artifact is {actual} bytes, above the {maximum}-byte bound")]
    ArtifactTooLarge { actual: usize, maximum: usize },
    #[error("artifact registry snapshot is {actual} bytes, above the {maximum}-byte bound")]
    SnapshotTooLarge { actual: usize, maximum: usize },
    #[error("artifact registry has reached its {maximum}-record limit")]
    Full { maximum: usize },
    #[error("artifact registry content digest conflict for {digest}")]
    Conflict { digest: String },
    #[error("artifact registry record {digest} was not found")]
    NotFound { digest: String },
    #[error("artifact registry JSON could not be canonicalised: {0}")]
    Canonicalisation(String),
    #[error("artifact registry snapshot is invalid: {0}")]
    InvalidSnapshot(String),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ArtifactRecord {
    pub content_digest: String,
    pub kind: String,
    pub subject_id: String,
    pub domains: Vec<String>,
    pub parent_digests: Vec<String>,
    pub declared_digest: Option<String>,
    pub verification: Value,
    pub artifact: Value,
}

#[derive(Debug, Clone, Default)]
pub struct ArtifactRegistry {
    generation: u64,
    records: BTreeMap<String, ArtifactRecord>,
}

impl ArtifactRegistry {
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

    /// Return bounded metadata copies for cross-store diagnostics.
    ///
    /// Artifact bodies are intentionally omitted from this projection. Callers that need a body
    /// must use `get` with its explicit digest, while consistency audits only need identity and
    /// kind information.
    pub fn records_for_audit(&self) -> Vec<ArtifactRecord> {
        self.records.values().cloned().collect()
    }

    /// Register one bounded artifact and preserve the verification method that admitted it.
    pub fn register(&mut self, request: &Value) -> Result<Value, ArtifactRegistryError> {
        let object = request
            .as_object()
            .ok_or(ArtifactRegistryError::RegistrationNotObject)?;
        let kind = required_text(object, "kind")?;
        if !ARTIFACT_KINDS.contains(&kind.as_str()) {
            return Err(ArtifactRegistryError::InvalidInput(format!(
                "kind must be one of {}",
                ARTIFACT_KINDS.join(", ")
            )));
        }
        let subject_id = required_text(object, "subject_id")?;
        let artifact = object
            .get("artifact")
            .ok_or_else(|| ArtifactRegistryError::InvalidInput("artifact is required".into()))?;
        let encoded_artifact = serde_json::to_vec(artifact)
            .map_err(|error| ArtifactRegistryError::Canonicalisation(error.to_string()))?;
        if encoded_artifact.len() > MAX_ARTIFACT_REGISTRY_BYTES {
            return Err(ArtifactRegistryError::ArtifactTooLarge {
                actual: encoded_artifact.len(),
                maximum: MAX_ARTIFACT_REGISTRY_BYTES,
            });
        }
        let domains = bounded_text_set(object, "domains", MAX_ARTIFACT_REGISTRY_DOMAINS)?;
        let parent_digests =
            bounded_digest_set(object, "parent_digests", MAX_ARTIFACT_REGISTRY_PARENTS)?;
        let content_digest = content_digest(artifact)?;
        let (declared_digest, verification) = verify_known_artifact(kind.as_str(), artifact)?;
        if let Some(requested_digest) = optional_digest(object, "declared_digest")? {
            if Some(requested_digest.clone()) != declared_digest {
                return Err(ArtifactRegistryError::InvalidInput(format!(
                    "declared_digest does not match the {} artifact's internal digest",
                    kind
                )));
            }
        }
        let candidate = ArtifactRecord {
            content_digest: content_digest.clone(),
            kind: kind.clone(),
            subject_id: subject_id.clone(),
            domains: domains.clone(),
            parent_digests: parent_digests.clone(),
            declared_digest: declared_digest.clone(),
            verification: verification.clone(),
            artifact: artifact.clone(),
        };
        if let Some(existing) = self.records.get(&content_digest) {
            if existing == &candidate {
                return Ok(register_report(&candidate, false, true, self));
            }
            return Err(ArtifactRegistryError::Conflict {
                digest: content_digest,
            });
        }
        if self.records.len() >= MAX_ARTIFACT_REGISTRY_RECORDS {
            return Err(ArtifactRegistryError::Full {
                maximum: MAX_ARTIFACT_REGISTRY_RECORDS,
            });
        }
        let mut candidate_registry = self.clone();
        candidate_registry
            .records
            .insert(content_digest.clone(), candidate.clone());
        candidate_registry.generation = candidate_registry.generation.saturating_add(1);
        candidate_registry.ensure_snapshot_bound()?;
        self.records = candidate_registry.records;
        self.generation = candidate_registry.generation;
        Ok(register_report(&candidate, true, false, self))
    }

    pub fn get(&self, digest: &str) -> Result<Value, ArtifactRegistryError> {
        validate_digest(digest, "content_digest")?;
        let record = self
            .records
            .get(digest)
            .ok_or_else(|| ArtifactRegistryError::NotFound {
                digest: digest.to_string(),
            })?;
        Ok(json!({
            "ok": true,
            "schema": ARTIFACT_REGISTRY_GET_SCHEMA_VERSION,
            "workflow": "artifact_registry_get",
            "record": record,
            "execution": "not_started",
            "guarantees": [
                "the returned content_digest identifies the exact indexed artifact JSON",
                "the verification method is retained instead of being inferred from presence"
            ],
            "does_not_claim": [
                "scientific, clinical, regulatory, publication, or external-effect validity",
                "that a declared parent is causally correct or externally available"
            ]
        }))
    }

    /// Query index rows without returning artifact bodies unless explicitly requested.
    pub fn query(
        &self,
        kind: Option<&str>,
        domain: Option<&str>,
        subject_id: Option<&str>,
        after: Option<&str>,
        max_items: usize,
        include_artifacts: bool,
    ) -> Result<Value, ArtifactRegistryError> {
        if !(1..=MAX_ARTIFACT_REGISTRY_QUERY_ITEMS).contains(&max_items) {
            return Err(ArtifactRegistryError::InvalidInput(format!(
                "max_items must be between 1 and {MAX_ARTIFACT_REGISTRY_QUERY_ITEMS}"
            )));
        }
        if let Some(kind) = kind {
            if !ARTIFACT_KINDS.contains(&kind) {
                return Err(ArtifactRegistryError::InvalidInput(format!(
                    "kind must be one of {}",
                    ARTIFACT_KINDS.join(", ")
                )));
            }
        }
        let mut rows = Vec::new();
        let mut has_more = false;
        for (digest, record) in self
            .records
            .iter()
            .filter(|(digest, _)| after.is_none_or(|cursor| digest.as_str() > cursor))
        {
            if kind.is_some_and(|value| value != record.kind) {
                continue;
            }
            if domain.is_some_and(|value| !record.domains.iter().any(|item| item == value)) {
                continue;
            }
            if subject_id.is_some_and(|value| value != record.subject_id) {
                continue;
            }
            if rows.len() >= max_items {
                has_more = true;
                break;
            }
            let mut row = json!({
                "content_digest": digest,
                "kind": record.kind,
                "subject_id": record.subject_id,
                "domains": record.domains,
                "parent_digests": record.parent_digests,
                "declared_digest": record.declared_digest,
                "verification": record.verification,
            });
            if include_artifacts {
                row["artifact"] = record.artifact.clone();
            }
            rows.push(row);
        }
        let next_after = if has_more {
            rows.last()
                .and_then(|row| row.get("content_digest"))
                .cloned()
                .unwrap_or(Value::Null)
        } else {
            Value::Null
        };
        Ok(json!({
            "ok": true,
            "schema": ARTIFACT_REGISTRY_QUERY_SCHEMA_VERSION,
            "workflow": "artifact_registry_query",
            "filters": {
                "kind": kind,
                "domain": domain,
                "subject_id": subject_id,
                "after": after,
                "max_items": max_items,
                "include_artifacts": include_artifacts
            },
            "registry_generation": self.generation,
            "registry_size": self.records.len(),
            "rows": rows,
            "next_after": next_after,
            "has_more": has_more,
            "execution": "not_started",
            "guarantees": [
                "rows are ordered by exact content_digest",
                "filters are applied to explicit registration metadata",
                "unregistered artifacts are absent rather than treated as verified"
            ],
            "does_not_claim": [
                "absence from this bounded registry means an artifact never existed",
                "a valid integrity check establishes domain truth"
            ]
        }))
    }

    /// Report the visible parent graph while keeping missing parents distinct from empty parents.
    pub fn lineage(&self, digest: &str) -> Result<Value, ArtifactRegistryError> {
        validate_digest(digest, "content_digest")?;
        if !self.records.contains_key(digest) {
            return Err(ArtifactRegistryError::NotFound {
                digest: digest.to_string(),
            });
        }
        let mut pending = VecDeque::from([digest.to_string()]);
        let mut visited = BTreeSet::new();
        let mut nodes = Vec::new();
        let mut missing = BTreeSet::new();
        let mut cycles = BTreeSet::new();
        while let Some(current) = pending.pop_front() {
            if !visited.insert(current.clone()) {
                cycles.insert(current);
                continue;
            }
            if visited.len() > MAX_ARTIFACT_REGISTRY_LINEAGE_NODES {
                return Err(ArtifactRegistryError::InvalidInput(format!(
                    "lineage exceeds the {}-node bound",
                    MAX_ARTIFACT_REGISTRY_LINEAGE_NODES
                )));
            }
            let record =
                self.records
                    .get(&current)
                    .ok_or_else(|| ArtifactRegistryError::NotFound {
                        digest: current.clone(),
                    })?;
            let mut present = Vec::new();
            let mut absent = Vec::new();
            for parent in &record.parent_digests {
                if self.records.contains_key(parent) {
                    present.push(parent.clone());
                    pending.push_back(parent.clone());
                } else {
                    absent.push(parent.clone());
                    missing.insert(parent.clone());
                }
            }
            nodes.push(json!({
                "content_digest": current,
                "kind": record.kind,
                "subject_id": record.subject_id,
                "parents_present": present,
                "parents_missing": absent,
                "verification": record.verification,
            }));
        }
        Ok(json!({
            "ok": true,
            "schema": ARTIFACT_REGISTRY_LINEAGE_SCHEMA_VERSION,
            "workflow": "artifact_registry_lineage",
            "root": digest,
            "nodes": nodes,
            "missing_parent_digests": missing.into_iter().collect::<Vec<_>>(),
            "cycles": cycles.into_iter().collect::<Vec<_>>(),
            "bounded": true,
            "execution": "not_started",
            "guarantees": [
                "present parents are traversed only through this registry",
                "missing parents remain explicit and are never replaced with an empty list",
                "cycles are reported rather than recursively followed without a bound"
            ],
            "does_not_claim": [
                "parent presence proves causal provenance or scientific validity",
                "the registry is a complete view of every artifact in the workspace"
            ]
        }))
    }

    /// Produce a digest-protected registry checkpoint.
    pub fn snapshot(&self) -> Result<Value, ArtifactRegistryError> {
        let mut document = json!({
            "schema": ARTIFACT_REGISTRY_SCHEMA_VERSION,
            "generation": self.generation,
            "record_count": self.records.len(),
            "records": self.records.values().collect::<Vec<_>>(),
            "retention": {
                "max_records": MAX_ARTIFACT_REGISTRY_RECORDS,
                "max_bytes": MAX_ARTIFACT_REGISTRY_BYTES,
                "max_parents": MAX_ARTIFACT_REGISTRY_PARENTS,
                "max_lineage_nodes": MAX_ARTIFACT_REGISTRY_LINEAGE_NODES
            },
            "execution": "not_started"
        });
        document["state_digest"] = Value::String(content_digest(&document)?);
        self.ensure_encoded_bound(&document)?;
        Ok(document)
    }

    /// Restore and independently re-check every indexed artifact and the outer snapshot digest.
    pub fn from_snapshot(document: &Value) -> Result<Self, ArtifactRegistryError> {
        let object = document.as_object().ok_or_else(|| {
            ArtifactRegistryError::InvalidSnapshot("snapshot must be an object".into())
        })?;
        let encoded = serde_json::to_vec(document)
            .map_err(|error| ArtifactRegistryError::Canonicalisation(error.to_string()))?;
        if encoded.len() > MAX_ARTIFACT_REGISTRY_BYTES {
            return Err(ArtifactRegistryError::SnapshotTooLarge {
                actual: encoded.len(),
                maximum: MAX_ARTIFACT_REGISTRY_BYTES,
            });
        }
        if object.get("schema").and_then(Value::as_str) != Some(ARTIFACT_REGISTRY_SCHEMA_VERSION) {
            return Err(ArtifactRegistryError::InvalidSnapshot(
                "schema is invalid".into(),
            ));
        }
        let claimed = required_digest(object, "state_digest")?;
        let mut unsigned = document.clone();
        unsigned
            .as_object_mut()
            .expect("snapshot object was checked above")
            .remove("state_digest");
        let recomputed = content_digest(&unsigned)?;
        if claimed != recomputed {
            return Err(ArtifactRegistryError::InvalidSnapshot(
                "state_digest does not match snapshot contents".into(),
            ));
        }
        let generation = object
            .get("generation")
            .and_then(Value::as_u64)
            .ok_or_else(|| {
                ArtifactRegistryError::InvalidSnapshot("generation is invalid".into())
            })?;
        let rows = object
            .get("records")
            .and_then(Value::as_array)
            .ok_or_else(|| {
                ArtifactRegistryError::InvalidSnapshot("records must be an array".into())
            })?;
        if rows.len() > MAX_ARTIFACT_REGISTRY_RECORDS {
            return Err(ArtifactRegistryError::Full {
                maximum: MAX_ARTIFACT_REGISTRY_RECORDS,
            });
        }
        if object.get("record_count").and_then(Value::as_u64) != Some(rows.len() as u64) {
            return Err(ArtifactRegistryError::InvalidSnapshot(
                "record_count does not match records".into(),
            ));
        }
        let mut registry = Self {
            generation,
            records: BTreeMap::new(),
        };
        for row in rows {
            let record: ArtifactRecord = serde_json::from_value(row.clone()).map_err(|error| {
                ArtifactRegistryError::InvalidSnapshot(format!("record is invalid: {error}"))
            })?;
            validate_record(&record)?;
            let actual_content_digest = content_digest(&record.artifact)?;
            if actual_content_digest != record.content_digest {
                return Err(ArtifactRegistryError::InvalidSnapshot(format!(
                    "record content_digest {} does not match artifact bytes",
                    record.content_digest
                )));
            }
            let (declared, verification) = verify_known_artifact(&record.kind, &record.artifact)?;
            if declared != record.declared_digest || verification != record.verification {
                return Err(ArtifactRegistryError::InvalidSnapshot(format!(
                    "record {} verification projection does not match its artifact",
                    record.content_digest
                )));
            }
            if registry
                .records
                .insert(record.content_digest.clone(), record)
                .is_some()
            {
                return Err(ArtifactRegistryError::InvalidSnapshot(
                    "snapshot contains duplicate content digests".into(),
                ));
            }
        }
        registry.ensure_snapshot_bound()?;
        Ok(registry)
    }

    fn ensure_snapshot_bound(&self) -> Result<(), ArtifactRegistryError> {
        let document = self.snapshot()?;
        self.ensure_encoded_bound(&document)
    }

    fn ensure_encoded_bound(&self, document: &Value) -> Result<(), ArtifactRegistryError> {
        let bytes = serde_json::to_vec(document)
            .map_err(|error| ArtifactRegistryError::Canonicalisation(error.to_string()))?;
        if bytes.len() > MAX_ARTIFACT_REGISTRY_BYTES {
            return Err(ArtifactRegistryError::SnapshotTooLarge {
                actual: bytes.len(),
                maximum: MAX_ARTIFACT_REGISTRY_BYTES,
            });
        }
        Ok(())
    }
}

fn register_report(
    record: &ArtifactRecord,
    created: bool,
    already_present: bool,
    registry: &ArtifactRegistry,
) -> Value {
    json!({
        "ok": true,
        "schema": ARTIFACT_REGISTRY_REGISTER_SCHEMA_VERSION,
        "workflow": "artifact_registry_register",
        "content_digest": record.content_digest,
        "kind": record.kind,
        "subject_id": record.subject_id,
        "created": created,
        "already_present": already_present,
        "registry_generation": registry.generation,
        "registry_size": registry.records.len(),
        "verification": record.verification,
        "declared_digest": record.declared_digest,
        "execution": "not_started",
        "guarantees": [
            "known artifact formats are independently checked before registration",
            "re-registering the exact record is idempotent",
            "a digest collision with different metadata is refused"
        ],
        "does_not_claim": [
            "artifact integrity establishes scientific, clinical, or release validity",
            "parent_digests are complete external provenance"
        ]
    })
}

fn verify_known_artifact(
    kind: &str,
    artifact: &Value,
) -> Result<(Option<String>, Value), ArtifactRegistryError> {
    match kind {
        "mission_evidence_bundle" => {
            let verification = verify_mission_evidence_bundle(artifact).map_err(|error| {
                ArtifactRegistryError::InvalidInput(format!(
                    "mission evidence bundle verification failed: {error}"
                ))
            })?;
            let valid = verification.get("valid").and_then(Value::as_bool) == Some(true);
            if !valid {
                return Err(ArtifactRegistryError::InvalidInput(
                    "mission evidence bundle verification did not return valid=true".into(),
                ));
            }
            let declared = required_digest(
                artifact.as_object().ok_or_else(|| {
                    ArtifactRegistryError::InvalidInput("evidence bundle must be an object".into())
                })?,
                "bundle_digest",
            )?;
            Ok((
                Some(declared.clone()),
                json!({
                    "state": "verified_integrity",
                    "method": "mission_evidence_bundle",
                    "bundle_digest": declared,
                    "verification_schema": verification.get("schema")
                }),
            ))
        }
        "workflow_reconciliation" => {
            let object = artifact.as_object().ok_or_else(|| {
                ArtifactRegistryError::InvalidInput(
                    "workflow reconciliation must be an object".into(),
                )
            })?;
            let declared = required_digest(object, "reconciliation_digest")?;
            let mut unsigned = artifact.clone();
            unsigned
                .as_object_mut()
                .expect("reconciliation object was checked above")
                .remove("reconciliation_digest");
            // `artifact_registry` is a transport projection attached after the canonical
            // reconciliation report has been verified. It is deliberately excluded from the
            // reconciliation's own digest contract so the returned report can be imported again
            // without creating a second semantic record.
            unsigned
                .as_object_mut()
                .expect("reconciliation object was checked above")
                .remove("artifact_registry");
            let recomputed = content_digest(&unsigned)?;
            if declared != recomputed {
                return Err(ArtifactRegistryError::InvalidInput(
                    "reconciliation_digest does not match the record contents".into(),
                ));
            }
            Ok((
                Some(declared.clone()),
                json!({
                    "state": "verified_integrity",
                    "method": "workflow_reconciliation_digest",
                    "reconciliation_digest": declared
                }),
            ))
        }
        "domain_report"
            if artifact.get("schema").and_then(Value::as_str)
                == Some(DOMAIN_REPORT_SCHEMA_VERSION) =>
        {
            validate_domain_report(artifact).map_err(|error| {
                ArtifactRegistryError::InvalidInput(format!(
                    "domain report verification failed: {error}"
                ))
            })?;
            Ok((
                None,
                json!({
                    "state": "verified_integrity",
                    "method": "domain_report_projection",
                    "schema": DOMAIN_REPORT_SCHEMA_VERSION
                }),
            ))
        }
        "domain_evidence_harmonization"
            if artifact.get("schema").and_then(Value::as_str)
                == Some(DOMAIN_EVIDENCE_HARMONIZATION_SCHEMA_VERSION) =>
        {
            validate_domain_evidence_harmonization(artifact).map_err(|error| {
                ArtifactRegistryError::InvalidInput(format!(
                    "domain evidence harmonization verification failed: {error}"
                ))
            })?;
            Ok((
                None,
                json!({
                    "state": "verified_integrity",
                    "method": "domain_evidence_harmonization",
                    "schema": DOMAIN_EVIDENCE_HARMONIZATION_SCHEMA_VERSION
                }),
            ))
        }
        "domain_evidence_intake"
            if artifact.get("schema").and_then(Value::as_str)
                == Some(DOMAIN_EVIDENCE_INTAKE_SCHEMA_VERSION) =>
        {
            validate_domain_evidence_intake(artifact).map_err(|error| {
                ArtifactRegistryError::InvalidInput(format!(
                    "domain evidence intake verification failed: {error}"
                ))
            })?;
            Ok((
                None,
                json!({
                    "state": "verified_integrity",
                    "method": "domain_evidence_intake",
                    "schema": DOMAIN_EVIDENCE_INTAKE_SCHEMA_VERSION
                }),
            ))
        }
        _ => Ok((
            None,
            json!({
                "state": "content_digest_verified",
                "method": "canonical_sha256",
                "semantic_verification": "not_run"
            }),
        )),
    }
}

fn required_text(
    object: &Map<String, Value>,
    field: &str,
) -> Result<String, ArtifactRegistryError> {
    let value = object
        .get(field)
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| ArtifactRegistryError::InvalidInput(format!("{field} must be non-empty")))?;
    if value.len() > MAX_ARTIFACT_REGISTRY_TEXT_BYTES {
        return Err(ArtifactRegistryError::InvalidInput(format!(
            "{field} exceeds the {MAX_ARTIFACT_REGISTRY_TEXT_BYTES}-byte bound"
        )));
    }
    Ok(value.to_string())
}

fn bounded_text_set(
    object: &Map<String, Value>,
    field: &str,
    maximum: usize,
) -> Result<Vec<String>, ArtifactRegistryError> {
    let Some(value) = object.get(field) else {
        return Ok(Vec::new());
    };
    let values = value
        .as_array()
        .ok_or_else(|| ArtifactRegistryError::InvalidInput(format!("{field} must be an array")))?;
    if values.len() > maximum {
        return Err(ArtifactRegistryError::InvalidInput(format!(
            "{field} exceeds the {maximum}-item bound"
        )));
    }
    let mut result = BTreeSet::new();
    for (index, value) in values.iter().enumerate() {
        let text = value
            .as_str()
            .filter(|text| !text.trim().is_empty())
            .ok_or_else(|| {
                ArtifactRegistryError::InvalidInput(format!(
                    "{field}[{index}] must be non-empty text"
                ))
            })?;
        if text.len() > MAX_ARTIFACT_REGISTRY_TEXT_BYTES {
            return Err(ArtifactRegistryError::InvalidInput(format!(
                "{field}[{index}] exceeds the text bound"
            )));
        }
        result.insert(text.to_string());
    }
    Ok(result.into_iter().collect())
}

fn bounded_digest_set(
    object: &Map<String, Value>,
    field: &str,
    maximum: usize,
) -> Result<Vec<String>, ArtifactRegistryError> {
    let values = bounded_text_set(object, field, maximum)?;
    for value in &values {
        validate_digest(value, field)?;
    }
    Ok(values)
}

fn optional_digest(
    object: &Map<String, Value>,
    field: &str,
) -> Result<Option<String>, ArtifactRegistryError> {
    match object.get(field) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(value)) => {
            validate_digest(value, field)?;
            Ok(Some(value.clone()))
        }
        Some(_) => Err(ArtifactRegistryError::InvalidInput(format!(
            "{field} must be a digest string or null"
        ))),
    }
}

fn required_digest(
    object: &Map<String, Value>,
    field: &str,
) -> Result<String, ArtifactRegistryError> {
    let value = object
        .get(field)
        .and_then(Value::as_str)
        .ok_or_else(|| ArtifactRegistryError::InvalidSnapshot(format!("{field} is missing")))?;
    validate_digest(value, field)?;
    Ok(value.to_string())
}

fn validate_digest(value: &str, field: &str) -> Result<(), ArtifactRegistryError> {
    ContentHash::parse(value.to_string()).map_err(|_| {
        ArtifactRegistryError::InvalidInput(format!(
            "{field} must be a lowercase 64-character SHA-256 digest"
        ))
    })?;
    Ok(())
}

fn content_digest(value: &Value) -> Result<String, ArtifactRegistryError> {
    ContentHash::of_value(value)
        .map(|digest| digest.to_string())
        .map_err(|error| ArtifactRegistryError::Canonicalisation(error.to_string()))
}

fn validate_record(record: &ArtifactRecord) -> Result<(), ArtifactRegistryError> {
    if !ARTIFACT_KINDS.contains(&record.kind.as_str()) {
        return Err(ArtifactRegistryError::InvalidSnapshot(format!(
            "record kind {:?} is not supported",
            record.kind
        )));
    }
    if record.subject_id.trim().is_empty()
        || record.subject_id.len() > MAX_ARTIFACT_REGISTRY_TEXT_BYTES
    {
        return Err(ArtifactRegistryError::InvalidSnapshot(
            "record subject_id is invalid".into(),
        ));
    }
    if record.domains.len() > MAX_ARTIFACT_REGISTRY_DOMAINS
        || record.parent_digests.len() > MAX_ARTIFACT_REGISTRY_PARENTS
    {
        return Err(ArtifactRegistryError::InvalidSnapshot(
            "record metadata exceeds its bound".into(),
        ));
    }
    for parent in &record.parent_digests {
        validate_digest(parent, "parent_digest")?;
    }
    if let Some(declared) = &record.declared_digest {
        validate_digest(declared, "declared_digest")?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn artifact(kind: &str, subject: &str, body: Value) -> Value {
        json!({
            "kind": kind,
            "subject_id": subject,
            "domains": ["oncology", "genomics", "oncology"],
            "parent_digests": [],
            "artifact": body
        })
    }

    #[test]
    fn registers_idempotently_and_queries_explicit_metadata() {
        let mut registry = ArtifactRegistry::new();
        let request = artifact("domain_report", "mission-1", json!({"status": "review"}));
        let first = registry.register(&request).unwrap();
        let second = registry.register(&request).unwrap();
        assert_eq!(first["created"], true);
        assert_eq!(second["already_present"], true);
        assert_eq!(registry.generation(), 1);
        let query = registry
            .query(
                Some("domain_report"),
                Some("genomics"),
                Some("mission-1"),
                None,
                10,
                false,
            )
            .unwrap();
        assert_eq!(query["rows"].as_array().unwrap().len(), 1);
        assert_eq!(query["rows"][0]["domains"], json!(["genomics", "oncology"]));
    }

    #[test]
    fn rejects_digest_conflicts_and_invalid_known_artifacts() {
        let mut registry = ArtifactRegistry::new();
        let request = artifact("domain_report", "mission-1", json!({"status": "review"}));
        registry.register(&request).unwrap();
        let mut conflict = request.clone();
        conflict["domains"] = json!(["different"]);
        assert!(matches!(
            registry.register(&conflict),
            Err(ArtifactRegistryError::Conflict { .. })
        ));
        let invalid_evidence = artifact(
            "mission_evidence_bundle",
            "mission-2",
            json!({"not": "a bundle"}),
        );
        assert!(matches!(
            registry.register(&invalid_evidence),
            Err(ArtifactRegistryError::InvalidInput(_))
        ));
    }

    #[test]
    fn lineage_keeps_missing_parent_and_cycle_states_distinct() {
        let mut registry = ArtifactRegistry::new();
        let leaf = artifact("domain_report", "leaf", json!({"v": 1}));
        let leaf_report = registry.register(&leaf).unwrap();
        let leaf_digest = leaf_report["content_digest"].as_str().unwrap().to_string();
        let mut root = artifact("mission_report", "root", json!({"v": 2}));
        root["parent_digests"] = json!([leaf_digest, "f".repeat(64)]);
        let root_report = registry.register(&root).unwrap();
        let lineage = registry
            .lineage(root_report["content_digest"].as_str().unwrap())
            .unwrap();
        assert_eq!(lineage["nodes"].as_array().unwrap().len(), 2);
        assert_eq!(
            lineage["missing_parent_digests"].as_array().unwrap().len(),
            1
        );
        assert_eq!(lineage["cycles"], json!([]));
    }

    #[test]
    fn snapshot_round_trip_and_tamper_rejection_are_digest_bound() {
        let mut registry = ArtifactRegistry::new();
        registry
            .register(&artifact("evaluator_replay", "mission-1", json!({"v": 1})))
            .unwrap();
        let snapshot = registry.snapshot().unwrap();
        let restored = ArtifactRegistry::from_snapshot(&snapshot).unwrap();
        assert_eq!(restored.len(), 1);
        let mut tampered = snapshot;
        tampered["records"][0]["artifact"]["v"] = json!(2);
        assert!(matches!(
            ArtifactRegistry::from_snapshot(&tampered),
            Err(ArtifactRegistryError::InvalidSnapshot(_))
                | Err(ArtifactRegistryError::Canonicalisation(_))
        ));
    }
}
