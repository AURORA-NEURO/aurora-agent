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

use crate::adapter_execution_evidence::ADAPTER_EXECUTION_EVIDENCE_SCHEMA;
use crate::domain_decision_readiness::{
    validate_domain_decision_readiness, DOMAIN_DECISION_READINESS_SCHEMA_VERSION,
};
use crate::domain_evidence::{
    validate_domain_evidence_harmonization, DOMAIN_EVIDENCE_HARMONIZATION_SCHEMA_VERSION,
};
use crate::domain_evidence_intake::{
    validate_domain_evidence_intake, DOMAIN_EVIDENCE_INTAKE_SCHEMA_VERSION,
};
use crate::domain_evidence_provider::DOMAIN_EVIDENCE_PROVIDER_REPLAY_SCHEMA;
use crate::domain_evidence_provider_external::DOMAIN_EVIDENCE_PROVIDER_EXTERNAL_PAYLOAD_REPLAY_SCHEMA;
use crate::domain_evidence_provider_external::DOMAIN_EVIDENCE_PROVIDER_EXTERNAL_PAYLOAD_SCHEMA;
use crate::domain_evidence_provider_external_execution::DOMAIN_EVIDENCE_PROVIDER_EXTERNAL_PAYLOAD_EXECUTION_SCHEMA;
use crate::domain_evidence_provider_external_lineage::DOMAIN_EVIDENCE_PROVIDER_EXTERNAL_PAYLOAD_LINEAGE_SCHEMA;
use crate::domain_evidence_provider_handoff::DOMAIN_EVIDENCE_PROVIDER_HANDOFF_SCHEMA;
use crate::domain_evidence_source::{
    validate_domain_evidence_source_plan, DOMAIN_EVIDENCE_SOURCE_PLAN_SCHEMA_VERSION,
};
use crate::domain_report::{validate_domain_report, DOMAIN_REPORT_SCHEMA_VERSION};
use crate::evidence_bundle::verify_mission_evidence_bundle;
use crate::workflow_execution_evidence::{
    validate_workflow_execution_evidence, WORKFLOW_EXECUTION_EVIDENCE_SCHEMA_VERSION,
};
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
pub const ARTIFACT_REGISTRY_DOMAIN_EVIDENCE_POSTURE_SCHEMA_VERSION: &str =
    "bioprism-devplat-artifact-domain-evidence-posture/0.1";
pub const ARTIFACT_REGISTRY_DOMAIN_EVIDENCE_LINEAGE_SCHEMA_VERSION: &str =
    "bioprism-devplat-artifact-domain-evidence-lineage/0.1";
pub const ARTIFACT_REGISTRY_DOMAIN_DECISION_READINESS_QUERY_SCHEMA_VERSION: &str =
    "bioprism-devplat-artifact-domain-decision-readiness-query/0.1";
pub const ARTIFACT_REGISTRY_CONTROL_PLANE_READINESS_QUERY_SCHEMA_VERSION: &str =
    "bioprism-devplat-artifact-control-plane-readiness-query/0.1";
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
    "workflow_execution_evidence",
    "mission_report",
    "evaluator_replay",
    "domain_report",
    "domain_evidence_harmonization",
    "domain_evidence_intake",
    "domain_evidence_provider_replay",
    "domain_evidence_provider_handoff",
    "domain_evidence_provider_external_payload",
    "domain_evidence_provider_external_payload_replay",
    "domain_evidence_provider_external_payload_lineage_audit",
    "domain_evidence_provider_external_payload_execution_evidence",
    "domain_decision_readiness",
    "control_plane_readiness",
    "adapter_execution_evidence",
    "domain_evidence_source_plan",
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

    /// Summarize the explicitly declared evidence that is visible for one capability group.
    ///
    /// This is intentionally an advisory projection. A record contributes only when its
    /// registration metadata names one of the requested domains (case-normalized exact matching)
    /// or when its artifact body explicitly names the selected `group_id`. Subject labels,
    /// artifact kind names, and free-form text are never used as inferred domain bindings.
    pub fn domain_evidence_posture(&self, group_id: &str, domains: &[String]) -> Value {
        let requested_domains = domains
            .iter()
            .map(|domain| domain.trim())
            .filter(|domain| !domain.is_empty())
            .map(str::to_owned)
            .collect::<BTreeSet<_>>();
        let requested_normalized = requested_domains
            .iter()
            .map(|domain| normalize_domain_label(domain))
            .collect::<BTreeSet<_>>();
        let mut kind_counts = BTreeMap::<String, usize>::new();
        let mut family_counts = BTreeMap::<String, usize>::new();
        let mut verification_state_counts = BTreeMap::<String, usize>::new();
        let mut match_basis_counts = BTreeMap::<String, usize>::new();
        let mut matched_domain_labels = BTreeSet::new();
        let mut subjects = BTreeSet::new();
        let mut matching_record_count = 0usize;
        let mut integrity_verified_record_count = 0usize;
        let mut parent_linked_record_count = 0usize;

        for record in self.records.values() {
            let artifact_group_id = record
                .artifact
                .get("group_id")
                .and_then(Value::as_str)
                .filter(|value| *value == group_id);
            let intersecting_domains = record
                .domains
                .iter()
                .filter(|domain| requested_normalized.contains(&normalize_domain_label(domain)))
                .cloned()
                .collect::<BTreeSet<_>>();
            let domain_match = !intersecting_domains.is_empty();
            let group_match = artifact_group_id.is_some();
            if !domain_match && !group_match {
                continue;
            }

            matching_record_count += 1;
            subjects.insert(record.subject_id.clone());
            if !record.parent_digests.is_empty() {
                parent_linked_record_count += 1;
            }
            for domain in intersecting_domains {
                matched_domain_labels.insert(domain);
            }
            *kind_counts.entry(record.kind.clone()).or_default() += 1;
            *family_counts
                .entry(artifact_family(&record.kind).to_string())
                .or_default() += 1;
            let verification_state = record
                .verification
                .get("state")
                .and_then(Value::as_str)
                .unwrap_or("unknown")
                .to_string();
            if matches!(
                verification_state.as_str(),
                "verified_integrity" | "content_digest_verified"
            ) {
                integrity_verified_record_count += 1;
            }
            *verification_state_counts
                .entry(verification_state)
                .or_default() += 1;
            let match_basis = match (domain_match, group_match) {
                (true, true) => "declared_group_id_and_artifact_domain_intersection",
                (true, false) => "artifact_domain_intersection",
                (false, true) => "declared_group_id",
                (false, false) => unreachable!("unmatched artifacts are skipped above"),
            };
            *match_basis_counts
                .entry(match_basis.to_string())
                .or_default() += 1;
        }

        json!({
            "ok": true,
            "schema": ARTIFACT_REGISTRY_DOMAIN_EVIDENCE_POSTURE_SCHEMA_VERSION,
            "workflow": "artifact_registry_domain_evidence_posture",
            "group_id": group_id,
            "requested_domains": requested_domains,
            "registry_generation": self.generation,
            "registry_size": self.records.len(),
            "state": if matching_record_count > 0 { "observed" } else { "missing" },
            "matching_record_count": matching_record_count,
            "integrity_verified_record_count": integrity_verified_record_count,
            "kind_counts": kind_counts,
            "family_counts": family_counts,
            "verification_state_counts": verification_state_counts,
            "match_basis_counts": match_basis_counts,
            "subject_count": subjects.len(),
            "parent_linked_record_count": parent_linked_record_count,
            "matched_domain_labels": matched_domain_labels,
            "scope": "exact_declared_registration_domain_intersection_or_explicit_artifact_group_id",
            "readiness_claimed": false,
            "execution": "not_started",
            "guarantees": [
                "only digest-verified records already present in this bounded registry are counted",
                "domain labels are matched exactly after case normalization",
                "an artifact group binding is accepted only from its explicit group_id field"
            ],
            "limitations": [
                "absence is not proof that evidence never existed",
                "artifact presence and integrity do not establish scientific, clinical, safety, regulatory, or release validity",
                "the projection does not inspect free-form subject text or infer domain membership"
            ]
        })
    }

    /// Return bounded metadata copies for cross-store diagnostics.
    ///
    /// Artifact bodies are intentionally omitted from this projection. Callers that need a body
    /// must use `get` with its explicit digest, while consistency audits only need identity and
    /// kind information.
    pub fn records_for_audit(&self) -> Vec<ArtifactRecord> {
        self.records.values().cloned().collect()
    }

    /// Trace retained domain-evidence intake artifacts through exact digest references.
    ///
    /// This is a derived read model over the same durable artifact index; it deliberately does
    /// not create a second intake store. Each row exposes the canonical request, response, and
    /// intake digests, direct declared parents, direct retained children, and the distinction
    /// between a source plan's internal `plan_digest` and its indexed artifact content digest.
    /// That distinction matters because a plan digest identifies the canonical plan body while a
    /// content digest identifies the exact indexed record and is the only value traversable by
    /// the registry's parent graph.
    pub fn domain_evidence_lineage(&self, request: &Value) -> Result<Value, ArtifactRegistryError> {
        let object = request
            .as_object()
            .ok_or(ArtifactRegistryError::RegistrationNotObject)?;
        let content_digest = optional_digest(object, "content_digest")?;
        let group_id = optional_text(object, "group_id")?;
        let domain = optional_text(object, "domain")?;
        let subject_id = optional_text(object, "subject_id")?;
        let source_tool = optional_text(object, "source_tool")?;
        let outcome = optional_text(object, "outcome")?;
        let request_digest = optional_digest(object, "request_digest")?;
        let response_digest = optional_digest(object, "response_digest")?;
        let intake_digest = optional_digest(object, "intake_digest")?;
        let source_plan_digest = optional_digest(object, "source_plan_digest")?;
        let after = optional_digest(object, "after")?;
        let max_items = object
            .get("max_items")
            .map(|value| {
                value
                    .as_u64()
                    .ok_or_else(|| {
                        ArtifactRegistryError::InvalidInput("max_items must be an integer".into())
                    })
                    .and_then(|number| {
                        usize::try_from(number).map_err(|_| {
                            ArtifactRegistryError::InvalidInput("max_items is too large".into())
                        })
                    })
            })
            .transpose()?
            .unwrap_or(100);
        if !(1..=MAX_ARTIFACT_REGISTRY_QUERY_ITEMS).contains(&max_items) {
            return Err(ArtifactRegistryError::InvalidInput(format!(
                "max_items must be between 1 and {MAX_ARTIFACT_REGISTRY_QUERY_ITEMS}"
            )));
        }
        let include_children = object
            .get("include_children")
            .map(|value| {
                value.as_bool().ok_or_else(|| {
                    ArtifactRegistryError::InvalidInput("include_children must be a boolean".into())
                })
            })
            .transpose()?
            .unwrap_or(true);
        if let Some(outcome) = &outcome {
            if !matches!(
                outcome.as_str(),
                "observed" | "partial" | "refused" | "error" | "unknown"
            ) {
                return Err(ArtifactRegistryError::InvalidInput(
                    "outcome must be observed, partial, refused, error, or unknown".into(),
                ));
            }
        }
        if content_digest.is_some() && after.is_some() {
            return Err(ArtifactRegistryError::InvalidInput(
                "after cannot be combined with content_digest".into(),
            ));
        }

        let records = self.records.values().collect::<Vec<_>>();
        let root = if let Some(digest) = &content_digest {
            let record =
                self.records
                    .get(digest)
                    .ok_or_else(|| ArtifactRegistryError::NotFound {
                        digest: digest.clone(),
                    })?;
            if record.kind != "domain_evidence_intake" {
                return Err(ArtifactRegistryError::InvalidInput(format!(
                    "content_digest {digest} is {}, not domain_evidence_intake",
                    record.kind
                )));
            }
            Some(record)
        } else {
            None
        };
        let mut matching = records
            .into_iter()
            .filter(|record| record.kind == "domain_evidence_intake")
            .filter(|record| {
                root.is_none_or(|candidate| candidate.content_digest == record.content_digest)
            })
            .filter(|record| {
                after
                    .as_ref()
                    .is_none_or(|cursor| record.content_digest > *cursor)
            })
            .filter(|record| {
                group_id.as_ref().is_none_or(|value| {
                    record.artifact.get("group_id").and_then(Value::as_str) == Some(value.as_str())
                })
            })
            .filter(|record| {
                domain.as_ref().is_none_or(|value| {
                    record
                        .artifact
                        .get("domains")
                        .and_then(Value::as_array)
                        .is_some_and(|domains| {
                            domains
                                .iter()
                                .filter_map(Value::as_str)
                                .any(|candidate| candidate.eq_ignore_ascii_case(value))
                        })
                })
            })
            .filter(|record| {
                subject_id
                    .as_ref()
                    .is_none_or(|value| record.subject_id == *value)
            })
            .filter(|record| {
                source_tool.as_ref().is_none_or(|value| {
                    record.artifact.get("source_tool").and_then(Value::as_str)
                        == Some(value.as_str())
                })
            })
            .filter(|record| {
                outcome.as_ref().is_none_or(|value| {
                    record.artifact.get("outcome").and_then(Value::as_str) == Some(value.as_str())
                })
            })
            .filter(|record| {
                request_digest.as_ref().is_none_or(|value| {
                    record
                        .artifact
                        .get("request_digest")
                        .and_then(Value::as_str)
                        == Some(value.as_str())
                })
            })
            .filter(|record| {
                response_digest.as_ref().is_none_or(|value| {
                    record
                        .artifact
                        .get("response_digest")
                        .and_then(Value::as_str)
                        == Some(value.as_str())
                })
            })
            .filter(|record| {
                intake_digest.as_ref().is_none_or(|value| {
                    record.artifact.get("intake_digest").and_then(Value::as_str)
                        == Some(value.as_str())
                })
            })
            .filter(|record| {
                source_plan_digest.as_ref().is_none_or(|value| {
                    record
                        .artifact
                        .get("source_plan_digest")
                        .and_then(Value::as_str)
                        == Some(value.as_str())
                })
            })
            .collect::<Vec<_>>();
        let has_more = matching.len() > max_items;
        if has_more {
            matching.truncate(max_items);
        }
        let rows = matching
            .iter()
            .map(|record| domain_evidence_lineage_row(record, &self.records, include_children))
            .collect::<Vec<_>>();
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
            "schema": ARTIFACT_REGISTRY_DOMAIN_EVIDENCE_LINEAGE_SCHEMA_VERSION,
            "workflow": "artifact_registry_domain_evidence_lineage",
            "filters": {
                "content_digest": content_digest,
                "group_id": group_id,
                "domain": domain,
                "subject_id": subject_id,
                "source_tool": source_tool,
                "outcome": outcome,
                "request_digest": request_digest,
                "response_digest": response_digest,
                "intake_digest": intake_digest,
                "source_plan_digest": source_plan_digest,
                "after": after,
                "max_items": max_items,
                "include_children": include_children
            },
            "registry_generation": self.generation,
            "registry_size": self.records.len(),
            "rows": rows,
            "next_after": next_after,
            "has_more": has_more,
            "trace_scope": "domain_evidence_intake_direct_declared_parents_and_direct_retained_children",
            "execution": "not_started",
            "guarantees": [
                "request, response, and intake digests are read from independently validated canonical intake artifacts",
                "declared parent digests remain distinct from retained parent records and missing parents",
                "source plan identity reports both the canonical plan_digest and the indexed content digest when available",
                "reverse child links are computed only from exact declared parent content digests",
                "all rows come from the same bounded artifact registry used by artifact get and artifact lineage"
            ],
            "does_not_claim": [
                "a digest match proves that a source tool executed or that its response is true",
                "a retained source plan or child artifact proves causal provenance, scientific, clinical, provider, release, or readiness validity",
                "missing parents or children prove that an artifact never existed",
                "the bounded local registry is a complete view of external evidence or workspace history"
            ]
        }))
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

    /// Query retained structural decision-readiness audits without returning report packets by
    /// default. This is intentionally narrower than a generic artifact query: callers can find
    /// a posture by its explicit state or policy result while the exact audit remains available
    /// through its content digest. Registry presence, policy satisfaction, and a ready-for-human-
    /// review state never become domain truth or execution authorization.
    pub fn domain_decision_readiness_query(
        &self,
        subject_id: Option<&str>,
        decision_state: Option<&str>,
        policy_satisfied: Option<bool>,
        after: Option<&str>,
        max_items: usize,
        include_audits: bool,
    ) -> Result<Value, ArtifactRegistryError> {
        if !(1..=MAX_ARTIFACT_REGISTRY_QUERY_ITEMS).contains(&max_items) {
            return Err(ArtifactRegistryError::InvalidInput(format!(
                "max_items must be between 1 and {MAX_ARTIFACT_REGISTRY_QUERY_ITEMS}"
            )));
        }
        if let Some(state) = decision_state {
            if !matches!(
                state,
                "ready_for_human_review" | "review_required" | "incomplete" | "blocked"
            ) {
                return Err(ArtifactRegistryError::InvalidInput(
                    "decision_state must be ready_for_human_review, review_required, incomplete, or blocked"
                        .into(),
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
            if record.kind != "domain_decision_readiness"
                || subject_id.is_some_and(|value| value != record.subject_id)
                || decision_state.is_some_and(|value| {
                    record
                        .artifact
                        .get("decision_state")
                        .and_then(Value::as_str)
                        != Some(value)
                })
                || policy_satisfied.is_some_and(|value| {
                    record
                        .artifact
                        .get("policy_satisfied")
                        .and_then(Value::as_bool)
                        != Some(value)
                })
            {
                continue;
            }
            if rows.len() >= max_items {
                has_more = true;
                break;
            }
            let mut row = json!({
                "content_digest": digest,
                "audit_digest": record.artifact.get("digest"),
                "subject_id": record.subject_id,
                "domains": record.domains,
                "decision_state": record.artifact.get("decision_state"),
                "policy_satisfied": record.artifact.get("policy_satisfied"),
                "report_count": record.artifact.get("report_count"),
                "counts": record.artifact.get("counts"),
                "parent_digests": record.parent_digests,
                "verification": record.verification,
            });
            if include_audits {
                row["audit"] = record.artifact.clone();
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
            "schema": ARTIFACT_REGISTRY_DOMAIN_DECISION_READINESS_QUERY_SCHEMA_VERSION,
            "workflow": "artifact_registry_domain_decision_readiness_query",
            "filters": {
                "subject_id": subject_id,
                "decision_state": decision_state,
                "policy_satisfied": policy_satisfied,
                "after": after,
                "max_items": max_items,
                "include_audits": include_audits
            },
            "registry_generation": self.generation,
            "registry_size": self.records.len(),
            "rows": rows,
            "next_after": next_after,
            "has_more": has_more,
            "execution": "not_started",
            "guarantees": [
                "only exact digest-verified domain_decision_readiness artifacts are returned",
                "rows are ordered by retained artifact content digest",
                "full audit bodies are opt-in and can be fetched again by content_digest"
            ],
            "does_not_claim": [
                "a ready_for_human_review state proves a scientific, clinical, causal, regulatory, publication, release, or execution conclusion",
                "absence from this bounded registry proves that a readiness audit never existed",
                "artifact retention proves external provenance, identity, consent, or authority"
            ]
        }))
    }

    /// Query retained control-plane readiness projections without returning their full bodies by
    /// default. The projection is a digest-bound composition of independently evaluated evidence
    /// packets; its state never grants execution, release, scientific, clinical, or regulatory
    /// authority.
    pub fn control_plane_readiness_query(
        &self,
        subject_id: Option<&str>,
        control_plane_state: Option<&str>,
        policy_satisfied: Option<bool>,
        after: Option<&str>,
        max_items: usize,
        include_audits: bool,
    ) -> Result<Value, ArtifactRegistryError> {
        if !(1..=MAX_ARTIFACT_REGISTRY_QUERY_ITEMS).contains(&max_items) {
            return Err(ArtifactRegistryError::InvalidInput(format!(
                "max_items must be between 1 and {MAX_ARTIFACT_REGISTRY_QUERY_ITEMS}"
            )));
        }
        if let Some(state) = control_plane_state {
            if !matches!(
                state,
                "ready_for_human_review" | "review_required" | "incomplete" | "blocked"
            ) {
                return Err(ArtifactRegistryError::InvalidInput(
                    "control_plane_state must be ready_for_human_review, review_required, incomplete, or blocked".into(),
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
            if record.kind != "control_plane_readiness"
                || subject_id.is_some_and(|value| value != record.subject_id)
                || control_plane_state.is_some_and(|value| {
                    record
                        .artifact
                        .get("control_plane_state")
                        .and_then(Value::as_str)
                        != Some(value)
                })
                || policy_satisfied.is_some_and(|value| {
                    record
                        .artifact
                        .get("policy_satisfied")
                        .and_then(Value::as_bool)
                        != Some(value)
                })
            {
                continue;
            }
            if rows.len() >= max_items {
                has_more = true;
                break;
            }
            let mut row = json!({
                "content_digest": digest,
                "audit_digest": record.artifact.get("digest"),
                "subject_id": record.subject_id,
                "domains": record.domains,
                "control_plane_state": record.artifact.get("control_plane_state"),
                "policy_satisfied": record.artifact.get("policy_satisfied"),
                "component_states": record.artifact.get("component_states"),
                "component_count": record.artifact.get("component_count"),
                "parent_digests": record.parent_digests,
                "verification": record.verification,
            });
            if include_audits {
                row["audit"] = record.artifact.clone();
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
            "schema": ARTIFACT_REGISTRY_CONTROL_PLANE_READINESS_QUERY_SCHEMA_VERSION,
            "workflow": "artifact_registry_control_plane_readiness_query",
            "filters": {
                "subject_id": subject_id,
                "control_plane_state": control_plane_state,
                "policy_satisfied": policy_satisfied,
                "after": after,
                "max_items": max_items,
                "include_audits": include_audits
            },
            "registry_generation": self.generation,
            "registry_size": self.records.len(),
            "rows": rows,
            "next_after": next_after,
            "has_more": has_more,
            "execution": "not_started",
            "guarantees": [
                "only exact digest-verified control_plane_readiness artifacts are returned",
                "rows are ordered by retained artifact content digest",
                "component state remains separate from the authority of each source evidence packet",
                "full projection bodies are opt-in and can be fetched again by content_digest"
            ],
            "does_not_claim": [
                "a ready_for_human_review state proves scientific, clinical, release, deployment, or execution authority",
                "absence from this bounded registry proves that a projection never existed",
                "artifact retention proves external provenance, identity, consent, or approval"
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

fn normalize_domain_label(value: &str) -> String {
    value.trim().to_ascii_lowercase()
}

fn optional_text(
    object: &Map<String, Value>,
    field: &str,
) -> Result<Option<String>, ArtifactRegistryError> {
    match object.get(field) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(value)) if !value.trim().is_empty() => {
            if value.len() > MAX_ARTIFACT_REGISTRY_TEXT_BYTES {
                return Err(ArtifactRegistryError::InvalidInput(format!(
                    "{field} exceeds the {MAX_ARTIFACT_REGISTRY_TEXT_BYTES}-byte bound"
                )));
            }
            Ok(Some(value.clone()))
        }
        Some(_) => Err(ArtifactRegistryError::InvalidInput(format!(
            "{field} must be non-empty text or null"
        ))),
    }
}

fn domain_evidence_lineage_row(
    record: &ArtifactRecord,
    records: &BTreeMap<String, ArtifactRecord>,
    include_children: bool,
) -> Value {
    let artifact = &record.artifact;
    let source_plan_digest = artifact
        .get("source_plan_digest")
        .and_then(Value::as_str)
        .map(str::to_string);
    let declared_parents = record
        .parent_digests
        .iter()
        .map(|digest| {
            if let Some(parent) = records.get(digest) {
                json!({
                    "content_digest": digest,
                    "present": true,
                    "kind": parent.kind,
                    "subject_id": parent.subject_id,
                    "domains": parent.domains,
                    "declared_digest": parent.declared_digest,
                    "verification": parent.verification,
                    "relation": "declared_parent_content_digest"
                })
            } else {
                json!({
                    "content_digest": digest,
                    "present": false,
                    "kind": Value::Null,
                    "subject_id": Value::Null,
                    "domains": [],
                    "declared_digest": Value::Null,
                    "verification": Value::Null,
                    "relation": "declared_parent_content_digest"
                })
            }
        })
        .collect::<Vec<_>>();
    let source_plan_matches = source_plan_digest
        .as_ref()
        .map(|digest| {
            records
                .values()
                .filter(|candidate| {
                    candidate.kind == "domain_evidence_source_plan"
                        && candidate.artifact.get("plan_digest").and_then(Value::as_str)
                            == Some(digest.as_str())
                })
                .map(|candidate| {
                    json!({
                        "plan_digest": digest,
                        "content_digest": candidate.content_digest,
                        "subject_id": candidate.subject_id,
                        "domains": candidate.domains,
                        "parent_declared": record.parent_digests.contains(&candidate.content_digest),
                        "verification": candidate.verification
                    })
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let source_plan_parent_content_linked = source_plan_matches
        .iter()
        .any(|candidate| candidate.get("parent_declared").and_then(Value::as_bool) == Some(true));
    let source_plan_binding_state = match (
        source_plan_digest.as_ref(),
        source_plan_matches.is_empty(),
        source_plan_parent_content_linked,
    ) {
        (None, _, _) => "not_declared",
        (Some(_), true, _) => "declared_plan_digest_unresolved",
        (Some(_), false, true) => "retained_and_content_parented",
        (Some(_), false, false)
            if record
                .parent_digests
                .iter()
                .any(|parent| source_plan_digest.as_deref() == Some(parent.as_str())) =>
        {
            "retained_declared_digest_only"
        }
        (Some(_), false, false) => "retained_but_not_parented",
    };
    let children = if include_children {
        records
            .values()
            .filter(|candidate| {
                candidate.content_digest != record.content_digest
                    && candidate.parent_digests.contains(&record.content_digest)
            })
            .map(|candidate| {
                json!({
                    "content_digest": candidate.content_digest,
                    "kind": candidate.kind,
                    "subject_id": candidate.subject_id,
                    "domains": candidate.domains,
                    "declared_digest": candidate.declared_digest,
                    "verification": candidate.verification,
                    "relation": "direct_retained_child_content_digest"
                })
            })
            .collect::<Vec<_>>()
    } else {
        Vec::new()
    };
    let request_digest = artifact
        .get("request_digest")
        .cloned()
        .unwrap_or(Value::Null);
    let response_digest = artifact
        .get("response_digest")
        .cloned()
        .unwrap_or(Value::Null);
    let intake_digest = artifact
        .get("intake_digest")
        .cloned()
        .unwrap_or(Value::Null);
    json!({
        "content_digest": record.content_digest,
        "kind": record.kind,
        "subject_id": record.subject_id,
        "group_id": artifact.get("group_id"),
        "domains": artifact.get("domains"),
        "source_tool": artifact.get("source_tool"),
        "outcome": artifact.get("outcome"),
        "request_supplied": artifact.get("request_supplied"),
        "request_digest": request_digest,
        "response_digest": response_digest,
        "intake_digest": intake_digest,
        "artifact_lookup": format!("/v1/artifacts/{}", record.content_digest),
        "artifact_lineage_lookup": format!("/v1/artifacts/{}/lineage", record.content_digest),
        "source_plan": {
            "plan_digest": source_plan_digest,
            "binding_state": source_plan_binding_state,
            "matches": source_plan_matches,
            "content_parent_linked": source_plan_parent_content_linked
        },
        "parents": declared_parents,
        "children": children,
        "parent_count": record.parent_digests.len(),
        "present_parent_count": record.parent_digests.iter().filter(|digest| records.contains_key(*digest)).count(),
        "missing_parent_count": record.parent_digests.iter().filter(|digest| !records.contains_key(*digest)).count(),
        "child_count": if include_children { children.len() } else { 0 },
        "verification": record.verification,
        "lineage_scope": "direct_declared_parents_and_direct_retained_children",
        "readiness_claimed": false,
        "execution": "not_started"
    })
}

fn artifact_family(kind: &str) -> &'static str {
    match kind {
        "adapter_execution_evidence" => "adapter_execution",
        kind if kind.starts_with("domain_evidence_provider") => "provider",
        "domain_evidence_source_plan"
        | "domain_evidence_intake"
        | "domain_evidence_harmonization"
        | "domain_decision_readiness" => "source_or_harmonization",
        "domain_report" => "domain_report",
        "mission_evidence_bundle"
        | "workflow_reconciliation"
        | "workflow_execution_evidence"
        | "mission_report"
        | "evaluator_replay" => "workflow_or_mission",
        "external_reference" => "external_reference",
        _ => "other",
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
        "workflow_execution_evidence"
            if artifact.get("schema").and_then(Value::as_str)
                == Some(WORKFLOW_EXECUTION_EVIDENCE_SCHEMA_VERSION) =>
        {
            validate_workflow_execution_evidence(artifact).map_err(|error| {
                ArtifactRegistryError::InvalidInput(format!(
                    "workflow execution evidence verification failed: {error}"
                ))
            })?;
            let declared = required_digest(
                artifact.as_object().ok_or_else(|| {
                    ArtifactRegistryError::InvalidInput(
                        "workflow execution evidence must be an object".into(),
                    )
                })?,
                "evidence_digest",
            )?;
            Ok((
                Some(declared),
                json!({
                    "state": "verified_integrity",
                    "method": "workflow_execution_evidence",
                    "schema": WORKFLOW_EXECUTION_EVIDENCE_SCHEMA_VERSION
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
        "domain_decision_readiness"
            if artifact.get("schema").and_then(Value::as_str)
                == Some(DOMAIN_DECISION_READINESS_SCHEMA_VERSION) =>
        {
            validate_domain_decision_readiness(artifact).map_err(|error| {
                ArtifactRegistryError::InvalidInput(format!(
                    "domain decision-readiness verification failed: {error}"
                ))
            })?;
            Ok((
                None,
                json!({
                    "state": "verified_integrity",
                    "method": "domain_decision_readiness",
                    "schema": DOMAIN_DECISION_READINESS_SCHEMA_VERSION
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
        "domain_evidence_provider_replay" => {
            let object = artifact.as_object().ok_or_else(|| {
                ArtifactRegistryError::InvalidInput(
                    "domain evidence provider replay must be an object".into(),
                )
            })?;
            if object.get("schema").and_then(Value::as_str)
                != Some(DOMAIN_EVIDENCE_PROVIDER_REPLAY_SCHEMA)
            {
                return Err(ArtifactRegistryError::InvalidInput(
                    "domain evidence provider replay schema is unsupported".into(),
                ));
            }
            let declared = required_digest(object, "replay_digest")?;
            let mut unsigned = artifact.clone();
            unsigned
                .as_object_mut()
                .expect("replay object was checked above")
                .remove("replay_digest");
            let recomputed = content_digest(&unsigned)?;
            if declared != recomputed {
                return Err(ArtifactRegistryError::InvalidInput(
                    "replay_digest does not match the record contents".into(),
                ));
            }
            Ok((
                Some(declared.clone()),
                json!({
                    "state": "verified_integrity",
                    "method": "domain_evidence_provider_replay_digest",
                    "replay_digest": declared
                }),
            ))
        }
        "domain_evidence_provider_handoff" => {
            let object = artifact.as_object().ok_or_else(|| {
                ArtifactRegistryError::InvalidInput(
                    "domain evidence provider handoff must be an object".into(),
                )
            })?;
            if object.get("schema").and_then(Value::as_str)
                != Some(DOMAIN_EVIDENCE_PROVIDER_HANDOFF_SCHEMA)
            {
                return Err(ArtifactRegistryError::InvalidInput(
                    "domain evidence provider handoff schema is unsupported".into(),
                ));
            }
            let declared = required_digest(object, "handoff_digest")?;
            let mut unsigned = artifact.clone();
            unsigned
                .as_object_mut()
                .expect("handoff object was checked above")
                .remove("handoff_digest");
            let recomputed = content_digest(&unsigned)?;
            if declared != recomputed {
                return Err(ArtifactRegistryError::InvalidInput(
                    "handoff_digest does not match the record contents".into(),
                ));
            }
            Ok((
                Some(declared.clone()),
                json!({
                    "state": "verified_integrity",
                    "method": "domain_evidence_provider_handoff_digest",
                    "handoff_digest": declared
                }),
            ))
        }
        "domain_evidence_provider_external_payload" => {
            let object = artifact.as_object().ok_or_else(|| {
                ArtifactRegistryError::InvalidInput(
                    "domain evidence provider external payload receipt must be an object".into(),
                )
            })?;
            if object.get("schema").and_then(Value::as_str)
                != Some(DOMAIN_EVIDENCE_PROVIDER_EXTERNAL_PAYLOAD_SCHEMA)
            {
                return Err(ArtifactRegistryError::InvalidInput(
                    "domain evidence provider external payload receipt schema is unsupported"
                        .into(),
                ));
            }
            let declared = required_digest(object, "receipt_digest")?;
            let mut unsigned = artifact.clone();
            unsigned
                .as_object_mut()
                .expect("external payload receipt object was checked above")
                .remove("receipt_digest");
            let recomputed = content_digest(&unsigned)?;
            if declared != recomputed {
                return Err(ArtifactRegistryError::InvalidInput(
                    "receipt_digest does not match the record contents".into(),
                ));
            }
            Ok((
                Some(declared.clone()),
                json!({
                    "state": "verified_integrity",
                    "method": "domain_evidence_provider_external_payload_digest",
                    "receipt_digest": declared
                }),
            ))
        }
        "domain_evidence_provider_external_payload_replay" => {
            let object = artifact.as_object().ok_or_else(|| {
                ArtifactRegistryError::InvalidInput(
                    "domain evidence provider external payload replay must be an object".into(),
                )
            })?;
            if object.get("schema").and_then(Value::as_str)
                != Some(DOMAIN_EVIDENCE_PROVIDER_EXTERNAL_PAYLOAD_REPLAY_SCHEMA)
            {
                return Err(ArtifactRegistryError::InvalidInput(
                    "domain evidence provider external payload replay schema is unsupported".into(),
                ));
            }
            let declared = required_digest(object, "replay_digest")?;
            let mut unsigned = artifact.clone();
            unsigned
                .as_object_mut()
                .expect("external payload replay object was checked above")
                .remove("replay_digest");
            let recomputed = content_digest(&unsigned)?;
            if declared != recomputed {
                return Err(ArtifactRegistryError::InvalidInput(
                    "external payload replay_digest does not match the record contents".into(),
                ));
            }
            Ok((
                Some(declared.clone()),
                json!({
                    "state": "verified_integrity",
                    "method": "domain_evidence_provider_external_payload_replay_digest",
                    "replay_digest": declared
                }),
            ))
        }
        "domain_evidence_provider_external_payload_lineage_audit" => {
            let object = artifact.as_object().ok_or_else(|| {
                ArtifactRegistryError::InvalidInput(
                    "domain evidence provider external payload lineage audit must be an object"
                        .into(),
                )
            })?;
            if object.get("schema").and_then(Value::as_str)
                != Some(DOMAIN_EVIDENCE_PROVIDER_EXTERNAL_PAYLOAD_LINEAGE_SCHEMA)
            {
                return Err(ArtifactRegistryError::InvalidInput(
                    "domain evidence provider external payload lineage schema is unsupported"
                        .into(),
                ));
            }
            let declared = required_digest(object, "lineage_digest")?;
            let mut unsigned = artifact.clone();
            unsigned
                .as_object_mut()
                .expect("external payload lineage object was checked above")
                .remove("lineage_digest");
            let recomputed = content_digest(&unsigned)?;
            if declared != recomputed {
                return Err(ArtifactRegistryError::InvalidInput(
                    "external payload lineage_digest does not match the record contents".into(),
                ));
            }
            Ok((
                Some(declared.clone()),
                json!({
                    "state": "verified_integrity",
                    "method": "domain_evidence_provider_external_payload_lineage_digest",
                    "lineage_digest": declared
                }),
            ))
        }
        "domain_evidence_provider_external_payload_execution_evidence" => {
            let object = artifact.as_object().ok_or_else(|| {
                ArtifactRegistryError::InvalidInput(
                    "domain evidence provider external payload execution evidence must be an object"
                        .into(),
                )
            })?;
            if object.get("schema").and_then(Value::as_str)
                != Some(DOMAIN_EVIDENCE_PROVIDER_EXTERNAL_PAYLOAD_EXECUTION_SCHEMA)
            {
                return Err(ArtifactRegistryError::InvalidInput(
                    "domain evidence provider external payload execution schema is unsupported"
                        .into(),
                ));
            }
            let declared = required_digest(object, "evidence_digest")?;
            let mut unsigned = artifact.clone();
            unsigned
                .as_object_mut()
                .expect("external payload execution object was checked above")
                .remove("evidence_digest");
            let recomputed = content_digest(&unsigned)?;
            if declared != recomputed {
                return Err(ArtifactRegistryError::InvalidInput(
                    "external payload evidence_digest does not match the record contents".into(),
                ));
            }
            Ok((
                Some(declared.clone()),
                json!({
                    "state": "verified_integrity",
                    "method": "domain_evidence_provider_external_payload_execution_digest",
                    "evidence_digest": declared
                }),
            ))
        }
        "adapter_execution_evidence" => {
            let object = artifact.as_object().ok_or_else(|| {
                ArtifactRegistryError::InvalidInput(
                    "adapter execution evidence must be an object".into(),
                )
            })?;
            if object.get("schema").and_then(Value::as_str)
                != Some(ADAPTER_EXECUTION_EVIDENCE_SCHEMA)
            {
                return Err(ArtifactRegistryError::InvalidInput(
                    "adapter execution evidence schema is unsupported".into(),
                ));
            }
            let declared = required_digest(object, "evidence_digest")?;
            let mut unsigned = artifact.clone();
            unsigned
                .as_object_mut()
                .expect("adapter execution evidence object was checked above")
                .remove("evidence_digest");
            let recomputed = content_digest(&unsigned)?;
            if declared != recomputed {
                return Err(ArtifactRegistryError::InvalidInput(
                    "adapter execution evidence_digest does not match the record contents".into(),
                ));
            }
            Ok((
                Some(declared.clone()),
                json!({
                    "state": "verified_integrity",
                    "method": "adapter_execution_evidence_digest",
                    "evidence_digest": declared
                }),
            ))
        }
        "domain_evidence_source_plan"
            if artifact.get("schema").and_then(Value::as_str)
                == Some(DOMAIN_EVIDENCE_SOURCE_PLAN_SCHEMA_VERSION) =>
        {
            validate_domain_evidence_source_plan(artifact).map_err(|error| {
                ArtifactRegistryError::InvalidInput(format!(
                    "domain evidence source plan verification failed: {error}"
                ))
            })?;
            let digest = artifact
                .get("plan_digest")
                .and_then(Value::as_str)
                .map(str::to_string);
            Ok((
                digest,
                json!({
                    "state": "verified_integrity",
                    "method": "domain_evidence_source_plan",
                    "schema": DOMAIN_EVIDENCE_SOURCE_PLAN_SCHEMA_VERSION
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
    fn readiness_query_filters_verified_audits_and_keeps_bodies_opt_in() {
        let report = |group_id: &str, domain: &str, status: &str| {
            json!({
                "schema": "bioprism-devplat-domain-report/0.1",
                "workflow": "domain_report_project",
                "group_id": group_id,
                "domains": [domain],
                "subject_id": "subject-readiness-query",
                "source_tool": "modality_catalog",
                "report": {"observation": status},
                "claim_posture": {"status": status, "does_not_claim": ["truth"]},
                "parent_digests": ["a".repeat(64)],
                "readiness_claimed": false,
                "execution": "not_started",
                "guarantees": ["caller supplied"],
                "does_not_claim": ["scientific validity"]
            })
        };
        let first = report("biological_domains", "modalities", "observed");
        let second = report("biological_ir_and_query", "BioQL syntax", "derived");
        let first_digest = ContentHash::of_value(&first).unwrap().to_string();
        let second_digest = ContentHash::of_value(&second).unwrap().to_string();
        let audit = crate::audit_domain_decision_readiness(&json!({
            "subject_id": "subject-readiness-query",
            "claim": {"id": "claim-readiness-query", "statement": "opaque"},
            "reports": [first, second],
            "links": [
                {"report_index": 0, "report_digest": first_digest, "role": "supports"},
                {"report_index": 1, "report_digest": second_digest, "role": "qualifies", "note": "scope qualifier"}
            ],
            "policy": {
                "required_group_ids": ["biological_domains", "biological_ir_and_query"],
                "required_domains": ["modalities", "BioQL syntax"],
                "minimum_supporting_reports": 1,
                "minimum_qualifying_reports": 1,
                "require_lineage_parents": true
            }
        }))
        .unwrap();
        assert_eq!(audit["decision_state"], "ready_for_human_review");
        let audit_digest = audit["digest"].clone();
        let mut registry = ArtifactRegistry::new();
        let registration = registry
            .register(&json!({
                "kind": "domain_decision_readiness",
                "subject_id": "subject-readiness-query",
                "domains": ["modalities", "BioQL syntax"],
                "parent_digests": [first_digest, second_digest],
                "artifact": audit
            }))
            .unwrap();

        let summary = registry
            .domain_decision_readiness_query(
                Some("subject-readiness-query"),
                Some("ready_for_human_review"),
                Some(true),
                None,
                1,
                false,
            )
            .unwrap();
        assert_eq!(summary["rows"].as_array().unwrap().len(), 1);
        assert!(summary["rows"][0].get("audit").is_none());
        assert_eq!(
            summary["rows"][0]["content_digest"],
            registration["content_digest"]
        );

        let detailed = registry
            .domain_decision_readiness_query(
                Some("subject-readiness-query"),
                Some("ready_for_human_review"),
                Some(true),
                None,
                1,
                true,
            )
            .unwrap();
        assert_eq!(detailed["rows"][0]["audit"]["digest"], audit_digest);
        assert!(matches!(
            registry.domain_decision_readiness_query(None, Some("unknown"), None, None, 1, false),
            Err(ArtifactRegistryError::InvalidInput(_))
        ));
    }

    #[test]
    fn control_plane_query_is_cursor_ordered_and_survives_digest_checked_snapshot_restore() {
        let mut registry = ArtifactRegistry::new();
        let first = registry
            .register(&json!({
                "kind": "control_plane_readiness",
                "subject_id": "control-plane-query",
                "domains": ["oncology"],
                "parent_digests": ["a".repeat(64)],
                "artifact": {
                    "digest": "b".repeat(64),
                    "control_plane_state": "ready_for_human_review",
                    "policy_satisfied": true,
                    "component_states": {"domain_decision_readiness": {"state": "ready_for_human_review"}},
                    "component_count": 5
                }
            }))
            .unwrap();
        registry
            .register(&json!({
                "kind": "control_plane_readiness",
                "subject_id": "control-plane-query",
                "domains": ["oncology"],
                "parent_digests": [],
                "artifact": {
                    "digest": "c".repeat(64),
                    "control_plane_state": "incomplete",
                    "policy_satisfied": false,
                    "component_states": {"release": {"state": "incomplete"}},
                    "component_count": 5
                }
            }))
            .unwrap();
        let page = registry
            .control_plane_readiness_query(
                Some("control-plane-query"),
                Some("ready_for_human_review"),
                Some(true),
                None,
                1,
                false,
            )
            .unwrap();
        assert_eq!(page["rows"].as_array().unwrap().len(), 1);
        assert!(page["rows"][0].get("audit").is_none());
        assert_eq!(page["rows"][0]["content_digest"], first["content_digest"]);

        let snapshot = registry.snapshot().unwrap();
        let restored = ArtifactRegistry::from_snapshot(&snapshot).unwrap();
        let detailed = restored
            .control_plane_readiness_query(
                Some("control-plane-query"),
                Some("ready_for_human_review"),
                Some(true),
                None,
                1,
                true,
            )
            .unwrap();
        assert_eq!(
            detailed["rows"][0]["audit"]["digest"],
            json!("b".repeat(64))
        );
        assert!(matches!(
            restored.control_plane_readiness_query(None, Some("unknown"), None, None, 1, false),
            Err(ArtifactRegistryError::InvalidInput(_))
        ));
    }

    #[test]
    fn domain_evidence_posture_uses_only_exact_declared_bindings() {
        let mut registry = ArtifactRegistry::new();
        registry
            .register(&artifact(
                "domain_report",
                "mission-1",
                json!({"status": "review"}),
            ))
            .unwrap();
        registry
            .register(&json!({
                "kind": "external_reference",
                "subject_id": "mission-2",
                "domains": ["unrelated"],
                "parent_digests": [],
                "artifact": {"group_id": "biological_domains", "locator": "caller://ref-1"}
            }))
            .unwrap();
        registry
            .register(&json!({
                "kind": "external_reference",
                "subject_id": "mission-3",
                "domains": ["unrelated"],
                "parent_digests": [],
                "artifact": {"locator": "caller://ref-2"}
            }))
            .unwrap();

        let posture = registry.domain_evidence_posture(
            "biological_domains",
            &["ONCOLOGY".into(), "genomics".into()],
        );
        assert_eq!(posture["state"], "observed");
        assert_eq!(posture["matching_record_count"], 2);
        assert_eq!(posture["subject_count"], 2);
        assert_eq!(posture["parent_linked_record_count"], 0);
        assert_eq!(
            posture["match_basis_counts"]["artifact_domain_intersection"],
            1
        );
        assert_eq!(posture["match_basis_counts"]["declared_group_id"], 1);
        assert_eq!(posture["family_counts"]["domain_report"], 1);
        assert_eq!(posture["family_counts"]["external_reference"], 1);
        assert_eq!(posture["readiness_claimed"], false);
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
    fn domain_evidence_lineage_joins_exact_intake_digests_and_reverse_children() {
        let mut registry = ArtifactRegistry::new();
        let intake = crate::intake_domain_evidence(&json!({
            "group_id": "biological_domains",
            "domains": ["modalities"],
            "subject_id": "subject-lineage",
            "source_tool": "modality_catalog",
            "request": {"modality": "single_cell"},
            "response": {"modalities": ["single_cell"]},
            "outcome": "observed",
            "claim_posture": {
                "status": "observed",
                "does_not_claim": ["clinical validity"]
            }
        }))
        .unwrap();
        let intake_registration = json!({
            "kind": "domain_evidence_intake",
            "subject_id": "subject-lineage",
            "domains": ["modalities"],
            "parent_digests": [],
            "artifact": intake
        });
        let intake_report = registry.register(&intake_registration).unwrap();
        let intake_content_digest = intake_report["content_digest"].as_str().unwrap();
        registry
            .register(&json!({
                "kind": "external_reference",
                "subject_id": "subject-lineage",
                "domains": ["modalities"],
                "parent_digests": [intake_content_digest],
                "artifact": {"locator": "caller://child"}
            }))
            .unwrap();

        let trace = registry
            .domain_evidence_lineage(&json!({
                "content_digest": intake_content_digest,
                "request_digest": intake_registration["artifact"]["request_digest"],
                "include_children": true
            }))
            .unwrap();
        assert_eq!(
            trace["workflow"],
            "artifact_registry_domain_evidence_lineage"
        );
        assert_eq!(trace["rows"].as_array().unwrap().len(), 1);
        let row = &trace["rows"][0];
        assert_eq!(row["content_digest"], intake_content_digest);
        assert_eq!(
            row["request_digest"],
            intake_registration["artifact"]["request_digest"]
        );
        assert_eq!(
            row["response_digest"],
            intake_registration["artifact"]["response_digest"]
        );
        assert_eq!(row["present_parent_count"], 0);
        assert_eq!(row["missing_parent_count"], 0);
        assert_eq!(row["child_count"], 1);
        assert_eq!(row["source_plan"]["binding_state"], "not_declared");
        assert_eq!(
            row["children"][0]["relation"],
            "direct_retained_child_content_digest"
        );
    }

    #[test]
    fn domain_evidence_lineage_keeps_plan_digest_and_content_parent_posture_separate() {
        let mut registry = ArtifactRegistry::new();
        let intake = crate::intake_domain_evidence(&json!({
            "group_id": "biological_domains",
            "domains": ["modalities"],
            "subject_id": "subject-plan",
            "source_tool": "modality_catalog",
            "response": {"status": "partial"},
            "outcome": "partial",
            "source_plan_digest": "f".repeat(64),
            "claim_posture": {
                "status": "review_required",
                "does_not_claim": ["source provenance"]
            }
        }))
        .unwrap();
        let intake_parents = intake["parent_digests"].clone();
        let report = registry
            .register(&json!({
                "kind": "domain_evidence_intake",
                "subject_id": "subject-plan",
                "domains": ["modalities"],
                "parent_digests": intake_parents,
                "artifact": intake
            }))
            .unwrap();
        let trace = registry
            .domain_evidence_lineage(&json!({
                "content_digest": report["content_digest"]
            }))
            .unwrap();
        assert_eq!(
            trace["rows"][0]["source_plan"]["plan_digest"],
            "f".repeat(64)
        );
        assert_eq!(
            trace["rows"][0]["source_plan"]["binding_state"],
            "declared_plan_digest_unresolved"
        );
        assert_eq!(trace["rows"][0]["missing_parent_count"], 1);
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

    #[test]
    fn provider_replay_artifacts_reverify_their_declared_digest_on_restore() {
        let mut replay = json!({
            "schema": DOMAIN_EVIDENCE_PROVIDER_REPLAY_SCHEMA,
            "workflow": "domain_evidence_provider_replay_verify",
            "replay_status": "matched",
            "matched": true,
            "replay_digest": ""
        });
        let digest = {
            let mut unsigned = replay.clone();
            unsigned.as_object_mut().unwrap().remove("replay_digest");
            ContentHash::of_value(&unsigned).unwrap().to_string()
        };
        replay["replay_digest"] = json!(digest.clone());
        let mut request = artifact("domain_evidence_provider_replay", "provider-1", replay);
        request["declared_digest"] = json!(digest);
        let mut registry = ArtifactRegistry::new();
        let first = registry.register(&request).unwrap();
        assert_eq!(first["created"], true);
        let restored = ArtifactRegistry::from_snapshot(&registry.snapshot().unwrap()).unwrap();
        assert_eq!(restored.len(), 1);
    }

    #[test]
    fn provider_handoff_artifacts_reverify_their_declared_digest_on_restore() {
        let mut handoff = json!({
            "schema": DOMAIN_EVIDENCE_PROVIDER_HANDOFF_SCHEMA,
            "workflow": "domain_evidence_provider_connector_handoff",
            "group_id": "biological_domains",
            "domains": ["oncology"],
            "subject_id": "provider-1",
            "source_tool": "literature_bind_check",
            "provider": "pubmed",
            "connector_kind": "literature",
            "status": "prepared",
            "manifest": {"connector_id": "caller.pubmed", "transport": "caller_managed"},
            "manifest_digest": "a".repeat(64),
            "request_digest": null,
            "payload_digest": null,
            "source_plan_digest": null,
            "parent_digests": [],
            "attempt_id": null,
            "handoff_digest": ""
        });
        let digest = {
            let mut unsigned = handoff.clone();
            unsigned.as_object_mut().unwrap().remove("handoff_digest");
            ContentHash::of_value(&unsigned).unwrap().to_string()
        };
        handoff["handoff_digest"] = json!(digest.clone());
        let mut request = artifact("domain_evidence_provider_handoff", "provider-1", handoff);
        request["declared_digest"] = json!(digest);
        let mut registry = ArtifactRegistry::new();
        assert_eq!(registry.register(&request).unwrap()["created"], true);
        let restored = ArtifactRegistry::from_snapshot(&registry.snapshot().unwrap()).unwrap();
        assert_eq!(restored.len(), 1);
    }

    #[test]
    fn external_payload_receipts_reverify_their_declared_digest_on_restore() {
        let mut receipt = json!({
            "schema": DOMAIN_EVIDENCE_PROVIDER_EXTERNAL_PAYLOAD_SCHEMA,
            "workflow": "domain_evidence_provider_external_payload_receipt",
            "group_id": "biological_domains",
            "domains": ["oncology"],
            "subject_id": "provider-1",
            "source_tool": "literature_bind_check",
            "provider": "pubmed",
            "connector_kind": "literature",
            "handoff_digest": "a".repeat(64),
            "transfer_id": "transfer-1",
            "payload_digest": "b".repeat(64),
            "byte_length": 4096,
            "storage_backend": "object_store",
            "locator_kind": "opaque",
            "locator": "store://object/1",
            "content_type": null,
            "content_encoding": null,
            "request_digest": null,
            "parent_digests": [],
            "availability": "available",
            "retention": "durable",
            "attempt_id": null,
            "receipt_digest": ""
        });
        let digest = {
            let mut unsigned = receipt.clone();
            unsigned.as_object_mut().unwrap().remove("receipt_digest");
            ContentHash::of_value(&unsigned).unwrap().to_string()
        };
        receipt["receipt_digest"] = json!(digest.clone());
        let mut request = artifact(
            "domain_evidence_provider_external_payload",
            "provider-1",
            receipt,
        );
        request["declared_digest"] = json!(digest);
        let mut registry = ArtifactRegistry::new();
        assert_eq!(registry.register(&request).unwrap()["created"], true);
        let restored = ArtifactRegistry::from_snapshot(&registry.snapshot().unwrap()).unwrap();
        assert_eq!(restored.len(), 1);
    }

    #[test]
    fn external_payload_replays_reverify_their_declared_digest_on_restore() {
        let mut replay = json!({
            "schema": DOMAIN_EVIDENCE_PROVIDER_EXTERNAL_PAYLOAD_REPLAY_SCHEMA,
            "workflow": "domain_evidence_provider_external_payload_replay_verify",
            "replay_status": "mismatch",
            "matched": false,
            "replay_digest": ""
        });
        let digest = {
            let mut unsigned = replay.clone();
            unsigned.as_object_mut().unwrap().remove("replay_digest");
            ContentHash::of_value(&unsigned).unwrap().to_string()
        };
        replay["replay_digest"] = json!(digest.clone());
        let mut request = artifact(
            "domain_evidence_provider_external_payload_replay",
            "provider-1",
            replay,
        );
        request["declared_digest"] = json!(digest);
        let mut registry = ArtifactRegistry::new();
        assert_eq!(registry.register(&request).unwrap()["created"], true);
        let restored = ArtifactRegistry::from_snapshot(&registry.snapshot().unwrap()).unwrap();
        assert_eq!(restored.len(), 1);
    }

    #[test]
    fn external_payload_lineage_audits_reverify_their_declared_digest_on_restore() {
        let mut audit = json!({
            "schema": DOMAIN_EVIDENCE_PROVIDER_EXTERNAL_PAYLOAD_LINEAGE_SCHEMA,
            "workflow": "domain_evidence_provider_external_payload_lineage_audit",
            "lineage_status": "orphaned",
            "payload_binding_status": "not_available",
            "matches": {"handoff_present": false},
            "differences": ["handoff_not_retained"],
            "lineage_digest": ""
        });
        let digest = {
            let mut unsigned = audit.clone();
            unsigned.as_object_mut().unwrap().remove("lineage_digest");
            ContentHash::of_value(&unsigned).unwrap().to_string()
        };
        audit["lineage_digest"] = json!(digest.clone());
        let mut request = artifact(
            "domain_evidence_provider_external_payload_lineage_audit",
            "provider-1",
            audit,
        );
        request["declared_digest"] = json!(digest);
        let mut registry = ArtifactRegistry::new();
        assert_eq!(registry.register(&request).unwrap()["created"], true);
        let restored = ArtifactRegistry::from_snapshot(&registry.snapshot().unwrap()).unwrap();
        assert_eq!(restored.len(), 1);
    }

    #[test]
    fn external_payload_execution_evidence_reverifies_its_declared_digest_on_restore() {
        let mut evidence = json!({
            "schema": DOMAIN_EVIDENCE_PROVIDER_EXTERNAL_PAYLOAD_EXECUTION_SCHEMA,
            "workflow": "domain_evidence_provider_external_payload_execution_evidence",
            "evidence_status": "orphaned",
            "expected_receipt_digest": "a".repeat(64),
            "observed_receipt_digest": "b".repeat(64),
            "matches": {"receipt_present": false},
            "differences": ["receipt_not_retained"],
            "evidence_digest": ""
        });
        let digest = {
            let mut unsigned = evidence.clone();
            unsigned.as_object_mut().unwrap().remove("evidence_digest");
            ContentHash::of_value(&unsigned).unwrap().to_string()
        };
        evidence["evidence_digest"] = json!(digest.clone());
        let mut request = artifact(
            "domain_evidence_provider_external_payload_execution_evidence",
            "provider-1",
            evidence,
        );
        request["declared_digest"] = json!(digest);
        let mut registry = ArtifactRegistry::new();
        assert_eq!(registry.register(&request).unwrap()["created"], true);
        let restored = ArtifactRegistry::from_snapshot(&registry.snapshot().unwrap()).unwrap();
        assert_eq!(restored.len(), 1);
    }
}
