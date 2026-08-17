//! Bounded, restart-safe storage for verified mission evidence bundles.
//!
//! A verifier answers whether one artifact is internally consistent. A registry answers the
//! operational question that follows: which verified artifacts are available for inspection, and
//! can the answer survive a process restart? This module intentionally stores only bundles that
//! passed [`verify_mission_evidence_bundle`]. It never executes a mission, reruns an evaluator, or
//! upgrades a content digest into provenance, scientific validity, clinical meaning, or release
//! approval.

use crate::evidence_bundle::verify_mission_evidence_bundle;
use bioprism_ids::ContentHash;
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const EVIDENCE_REGISTRY_SCHEMA_VERSION: &str = "bioprism-devplat-evidence-bundle-registry/0.1";
pub const EVIDENCE_REGISTRY_IMPORT_SCHEMA_VERSION: &str =
    "bioprism-devplat-evidence-bundle-import/0.1";
pub const EVIDENCE_REGISTRY_QUERY_SCHEMA_VERSION: &str =
    "bioprism-devplat-evidence-bundle-query/0.1";
pub const MAX_EVIDENCE_REGISTRY_BUNDLES: usize = 256;
pub const MAX_EVIDENCE_REGISTRY_BYTES: usize = 32 * 1024 * 1024;
pub const MAX_EVIDENCE_REGISTRY_QUERY_ITEMS: usize = 256;

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum EvidenceRegistryError {
    #[error("evidence registry input is not an object")]
    NotObject,
    #[error("evidence bundle verification failed: {0}")]
    Verification(String),
    #[error("evidence registry has reached its {maximum}-bundle limit")]
    Full { maximum: usize },
    #[error("evidence registry snapshot is invalid: {0}")]
    InvalidSnapshot(String),
    #[error("evidence registry snapshot is {actual} bytes, above the {maximum}-byte bound")]
    SnapshotTooLarge { actual: usize, maximum: usize },
    #[error("evidence registry could not be canonicalised: {0}")]
    Canonicalisation(String),
}

#[derive(Debug, Clone, Default)]
pub struct EvidenceBundleRegistry {
    generation: u64,
    bundles: BTreeMap<String, Value>,
}

impl EvidenceBundleRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn generation(&self) -> u64 {
        self.generation
    }

    pub fn len(&self) -> usize {
        self.bundles.len()
    }

    pub fn is_empty(&self) -> bool {
        self.bundles.is_empty()
    }

    /// Return deterministic digest identities without exposing bundle bodies.
    pub fn digests_for_audit(&self) -> Vec<String> {
        self.bundles.keys().cloned().collect()
    }

    /// Import one bundle after independently verifying every integrity and retention check.
    pub fn import(&mut self, bundle: &Value) -> Result<Value, EvidenceRegistryError> {
        if !bundle.is_object() {
            return Err(EvidenceRegistryError::NotObject);
        }
        let verification = verify_mission_evidence_bundle(bundle)
            .map_err(|error| EvidenceRegistryError::Verification(error.to_string()))?;
        if verification.get("valid").and_then(Value::as_bool) != Some(true) {
            let failures = verification
                .get("failures")
                .and_then(Value::as_array)
                .map(|items| {
                    items
                        .iter()
                        .filter_map(Value::as_str)
                        .collect::<Vec<_>>()
                        .join(", ")
                })
                .unwrap_or_else(|| "verification_failed".into());
            return Err(EvidenceRegistryError::Verification(failures));
        }
        let digest = verification
            .get("bundle_digest")
            .and_then(Value::as_str)
            .ok_or_else(|| EvidenceRegistryError::Verification("bundle digest is missing".into()))?
            .to_string();
        let already_present = self
            .bundles
            .get(&digest)
            .is_some_and(|existing| existing == bundle);
        if !already_present && self.bundles.len() >= MAX_EVIDENCE_REGISTRY_BUNDLES {
            return Err(EvidenceRegistryError::Full {
                maximum: MAX_EVIDENCE_REGISTRY_BUNDLES,
            });
        }
        if !already_present {
            let mut candidate = self.clone();
            candidate.bundles.insert(digest.clone(), bundle.clone());
            candidate.generation = candidate.generation.saturating_add(1);
            candidate.ensure_snapshot_bound()?;
            self.bundles = candidate.bundles;
            self.generation = candidate.generation;
        }
        Ok(json!({
            "ok": true,
            "schema": EVIDENCE_REGISTRY_IMPORT_SCHEMA_VERSION,
            "workflow": "mission_evidence_bundle_import",
            "bundle_digest": digest,
            "created": !already_present,
            "already_present": already_present,
            "registry_generation": self.generation,
            "registry_size": self.bundles.len(),
            "execution": "not_started",
            "guarantees": [
                "only a bundle with a successful independent verification report is imported",
                "re-importing the same canonical bundle is idempotent",
                "import does not execute a mission, evaluator, domain tool, or external effect"
            ],
            "limitations": [
                "the registry is a bounded local evidence index rather than an external object store",
                "a verified digest does not establish provenance, scientific validity, or release approval"
            ]
        }))
    }

    pub fn get(&self, digest: &str) -> Option<Value> {
        self.bundles.get(digest).cloned()
    }

    /// Query deterministic index rows without returning full bundle bodies by default.
    pub fn query(
        &self,
        mission_id: Option<&str>,
        domain: Option<&str>,
        after: Option<&str>,
        max_items: usize,
        include_bundles: bool,
    ) -> Result<Value, EvidenceRegistryError> {
        if !(1..=MAX_EVIDENCE_REGISTRY_QUERY_ITEMS).contains(&max_items) {
            return Err(EvidenceRegistryError::InvalidSnapshot(format!(
                "max_items must be between 1 and {MAX_EVIDENCE_REGISTRY_QUERY_ITEMS}"
            )));
        }
        let mut rows = Vec::new();
        let mut has_more = false;
        for (digest, bundle) in self
            .bundles
            .iter()
            .filter(|(digest, _)| after.is_none_or(|cursor| digest.as_str() > cursor))
        {
            let index = index_row(digest, bundle)?;
            let mission_matches = mission_id
                .is_none_or(|value| index.get("mission_id").and_then(Value::as_str) == Some(value));
            let domain_matches = domain.is_none_or(|value| {
                index
                    .get("domains")
                    .and_then(Value::as_array)
                    .is_some_and(|domains| domains.iter().any(|row| row.as_str() == Some(value)))
            });
            if !mission_matches || !domain_matches {
                continue;
            }
            if rows.len() >= max_items {
                has_more = true;
                break;
            }
            let mut row = index;
            if include_bundles {
                row["bundle"] = bundle.clone();
            }
            rows.push(row);
        }
        let next_after = if has_more {
            rows.last()
                .and_then(|row| row.get("bundle_digest"))
                .cloned()
                .unwrap_or(Value::Null)
        } else {
            Value::Null
        };
        Ok(json!({
            "ok": true,
            "schema": EVIDENCE_REGISTRY_QUERY_SCHEMA_VERSION,
            "workflow": "mission_evidence_bundle_query",
            "filters": {
                "mission_id": mission_id,
                "domain": domain,
                "after": after,
                "max_items": max_items,
                "include_bundles": include_bundles
            },
            "registry_generation": self.generation,
            "registry_size": self.bundles.len(),
            "rows": rows,
            "next_after": next_after,
            "has_more": has_more,
            "execution": "not_started",
            "guarantees": [
                "rows are ordered by canonical bundle digest",
                "filters are applied to indexed mission and evaluator-domain metadata",
                "query does not execute a mission, evaluator, domain tool, or external effect"
            ],
            "limitations": [
                "results are bounded by the local registry retention policy",
                "absence from this registry is not evidence that an artifact never existed"
            ]
        }))
    }

    /// Return a digest-protected checkpoint document suitable for atomic persistence.
    pub fn snapshot(&self) -> Result<Value, EvidenceRegistryError> {
        let mut document = json!({
            "schema": EVIDENCE_REGISTRY_SCHEMA_VERSION,
            "generation": self.generation,
            "bundle_count": self.bundles.len(),
            "bundles": self.bundles.iter().map(|(digest, bundle)| json!({
                "bundle_digest": digest,
                "bundle": bundle
            })).collect::<Vec<_>>(),
            "retention": {
                "max_bundles": MAX_EVIDENCE_REGISTRY_BUNDLES,
                "max_bytes": MAX_EVIDENCE_REGISTRY_BYTES
            },
            "execution": "not_started"
        });
        let state_digest = snapshot_digest(&document)?;
        document["state_digest"] = Value::String(state_digest);
        self.ensure_encoded_bound(&document)?;
        Ok(document)
    }

    /// Restore a registry while re-verifying every imported bundle and the registry digest.
    pub fn from_snapshot(document: &Value) -> Result<Self, EvidenceRegistryError> {
        let object = document.as_object().ok_or_else(|| {
            EvidenceRegistryError::InvalidSnapshot("snapshot must be an object".into())
        })?;
        let encoded = serde_json::to_vec(document)
            .map_err(|error| EvidenceRegistryError::Canonicalisation(error.to_string()))?;
        if encoded.len() > MAX_EVIDENCE_REGISTRY_BYTES {
            return Err(EvidenceRegistryError::SnapshotTooLarge {
                actual: encoded.len(),
                maximum: MAX_EVIDENCE_REGISTRY_BYTES,
            });
        }
        if object.get("schema").and_then(Value::as_str) != Some(EVIDENCE_REGISTRY_SCHEMA_VERSION) {
            return Err(EvidenceRegistryError::InvalidSnapshot(
                "schema is invalid".into(),
            ));
        }
        let claimed_digest = object
            .get("state_digest")
            .and_then(Value::as_str)
            .ok_or_else(|| {
                EvidenceRegistryError::InvalidSnapshot("state_digest is missing".into())
            })?;
        let mut unsigned = document.clone();
        unsigned
            .as_object_mut()
            .expect("snapshot object was checked above")
            .remove("state_digest");
        let recomputed = snapshot_digest(&unsigned)?;
        if claimed_digest != recomputed {
            return Err(EvidenceRegistryError::InvalidSnapshot(
                "state_digest does not match snapshot contents".into(),
            ));
        }
        let generation = object
            .get("generation")
            .and_then(Value::as_u64)
            .ok_or_else(|| {
                EvidenceRegistryError::InvalidSnapshot("generation is invalid".into())
            })?;
        let rows = object
            .get("bundles")
            .and_then(Value::as_array)
            .ok_or_else(|| {
                EvidenceRegistryError::InvalidSnapshot("bundles must be an array".into())
            })?;
        if rows.len() > MAX_EVIDENCE_REGISTRY_BUNDLES {
            return Err(EvidenceRegistryError::Full {
                maximum: MAX_EVIDENCE_REGISTRY_BUNDLES,
            });
        }
        let mut registry = Self {
            generation,
            bundles: BTreeMap::new(),
        };
        for row in rows {
            let row_object = row.as_object().ok_or_else(|| {
                EvidenceRegistryError::InvalidSnapshot("bundle index row must be an object".into())
            })?;
            let digest = row_object
                .get("bundle_digest")
                .and_then(Value::as_str)
                .ok_or_else(|| {
                    EvidenceRegistryError::InvalidSnapshot("bundle_digest is missing".into())
                })?;
            let bundle = row_object.get("bundle").ok_or_else(|| {
                EvidenceRegistryError::InvalidSnapshot("bundle body is missing".into())
            })?;
            let verification = verify_mission_evidence_bundle(bundle).map_err(|error| {
                EvidenceRegistryError::InvalidSnapshot(format!(
                    "bundle {digest} is invalid: {error}"
                ))
            })?;
            if verification.get("valid").and_then(Value::as_bool) != Some(true)
                || verification.get("bundle_digest").and_then(Value::as_str) != Some(digest)
            {
                return Err(EvidenceRegistryError::InvalidSnapshot(format!(
                    "bundle {digest} failed digest verification"
                )));
            }
            if registry
                .bundles
                .insert(digest.to_string(), bundle.clone())
                .is_some()
            {
                return Err(EvidenceRegistryError::InvalidSnapshot(
                    "snapshot contains duplicate bundle digests".into(),
                ));
            }
        }
        if object.get("bundle_count").and_then(Value::as_u64) != Some(rows.len() as u64) {
            return Err(EvidenceRegistryError::InvalidSnapshot(
                "bundle_count does not match bundles".into(),
            ));
        }
        registry.ensure_snapshot_bound()?;
        Ok(registry)
    }

    fn ensure_snapshot_bound(&self) -> Result<(), EvidenceRegistryError> {
        let document = self.snapshot()?;
        self.ensure_encoded_bound(&document)
    }

    fn ensure_encoded_bound(&self, document: &Value) -> Result<(), EvidenceRegistryError> {
        let bytes = serde_json::to_vec(document)
            .map_err(|error| EvidenceRegistryError::Canonicalisation(error.to_string()))?;
        if bytes.len() > MAX_EVIDENCE_REGISTRY_BYTES {
            return Err(EvidenceRegistryError::SnapshotTooLarge {
                actual: bytes.len(),
                maximum: MAX_EVIDENCE_REGISTRY_BYTES,
            });
        }
        Ok(())
    }
}

fn snapshot_digest(document: &Value) -> Result<String, EvidenceRegistryError> {
    ContentHash::of_value(document)
        .map(|digest| digest.to_string())
        .map_err(|error| EvidenceRegistryError::Canonicalisation(error.to_string()))
}

fn index_row(digest: &str, bundle: &Value) -> Result<Value, EvidenceRegistryError> {
    let object = bundle.as_object().ok_or(EvidenceRegistryError::NotObject)?;
    let bytes = serde_json::to_vec(bundle)
        .map_err(|error| EvidenceRegistryError::Canonicalisation(error.to_string()))?;
    let mission_id = object
        .get("mission_id")
        .and_then(Value::as_str)
        .ok_or_else(|| EvidenceRegistryError::Verification("mission_id is missing".into()))?;
    let retention = object.get("retention").and_then(Value::as_object);
    let replay = object.get("evaluator_replay").and_then(Value::as_object);
    let mut domains = BTreeSet::new();
    let mut adapter_ids = BTreeSet::new();
    if let Some(bindings) = replay
        .and_then(|value| value.get("bindings"))
        .and_then(Value::as_array)
    {
        for binding in bindings {
            if let Some(domain) = binding.get("domain").and_then(Value::as_str) {
                domains.insert(domain.to_string());
            }
            if let Some(adapter_id) = binding.get("adapter_id").and_then(Value::as_str) {
                adapter_ids.insert(adapter_id.to_string());
            }
        }
    }
    if let Some(claims) = replay
        .and_then(|value| value.get("claims"))
        .and_then(Value::as_array)
    {
        for claim in claims {
            if let Some(bindings) = claim.get("bindings").and_then(Value::as_array) {
                for binding in bindings {
                    if let Some(domain) = binding.get("domain").and_then(Value::as_str) {
                        domains.insert(domain.to_string());
                    }
                    if let Some(adapter_id) = binding.get("adapter_id").and_then(Value::as_str) {
                        adapter_ids.insert(adapter_id.to_string());
                    }
                }
            }
        }
    }
    Ok(json!({
        "bundle_digest": digest,
        "mission_id": mission_id,
        "retention_mode": retention.and_then(|value| value.get("mode")).cloned().unwrap_or(Value::Null),
        "result_retained": retention.and_then(|value| value.get("result_retained")).cloned().unwrap_or(Value::Null),
        "result_included": retention.and_then(|value| value.get("result_included")).cloned().unwrap_or(Value::Null),
        "catalog_drift_status": object.get("catalog_drift").and_then(|value| value.get("status")).cloned().unwrap_or(Value::Null),
        "replay_status": replay.and_then(|value| value.get("replay_status")).cloned().unwrap_or(Value::Null),
        "domains": domains.into_iter().collect::<Vec<_>>(),
        "adapter_ids": adapter_ids.into_iter().collect::<Vec<_>>(),
        "trace_events": object.get("trace").and_then(Value::as_array).map_or(0, Vec::len),
        "bundle_bytes": bytes.len(),
        "execution": object.get("execution").cloned().unwrap_or(Value::Null)
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::evidence_bundle::MISSION_EVIDENCE_BUNDLE_SCHEMA_VERSION;

    fn bundle(mission_id: &str, domain: &str) -> Value {
        let mut value = json!({
            "schema": MISSION_EVIDENCE_BUNDLE_SCHEMA_VERSION,
            "workflow": "mission_evidence_bundle_export",
            "mission_id": mission_id,
            "retention": {"mode": "summary_only", "result_retained": false, "result_included": false},
            "result": null,
            "result_digest": null,
            "evaluator_replay": {"workflow": "mission_evaluator_replay_summary", "bindings": [{"domain": domain, "adapter_id": "adapter.one"}]},
            "catalog_drift": {"status": "unchanged"},
            "trace": [],
            "export": {"format": "json", "include_result": false, "include_trace": true, "trace_included": true, "digest_algorithm": "sha256", "execution": "not_started"},
            "execution": "not_started"
        });
        let digest = ContentHash::of_value(&value).unwrap().to_string();
        value["bundle_digest"] = Value::String(digest);
        value
    }

    #[test]
    fn imports_idempotently_and_queries_domain_index() {
        let mut registry = EvidenceBundleRegistry::new();
        let first = registry.import(&bundle("mission-one", "oncology")).unwrap();
        assert_eq!(first["created"], true);
        let second = registry
            .import(
                &registry
                    .get(first["bundle_digest"].as_str().unwrap())
                    .unwrap(),
            )
            .unwrap();
        assert_eq!(second["already_present"], true);
        let query = registry
            .query(Some("mission-one"), Some("oncology"), None, 10, false)
            .unwrap();
        assert_eq!(query["rows"].as_array().unwrap().len(), 1);
        assert_eq!(query["rows"][0]["domains"], json!(["oncology"]));
    }

    #[test]
    fn preserves_multi_domain_bindings_as_routing_metadata_without_semantic_inference() {
        let mut value = bundle("mission-multi-domain", "oncology");
        value["evaluator_replay"]["bindings"] = json!([
            {"domain": "oncology", "adapter_id": "adapter.oncology"},
            {"domain": "genomics", "adapter_id": "adapter.genomics"},
            {"domain": "imaging", "adapter_id": "adapter.imaging"},
            {"domain": "oncology", "adapter_id": "adapter.oncology"}
        ]);
        value.as_object_mut().unwrap().remove("bundle_digest");
        value["bundle_digest"] = Value::String(ContentHash::of_value(&value).unwrap().to_string());
        let mut registry = EvidenceBundleRegistry::new();
        registry.import(&value).unwrap();
        for domain in ["oncology", "genomics", "imaging"] {
            let page = registry.query(None, Some(domain), None, 10, false).unwrap();
            assert_eq!(page["rows"].as_array().unwrap().len(), 1);
            assert!(page["rows"][0]["domains"]
                .as_array()
                .unwrap()
                .iter()
                .any(|item| item.as_str() == Some(domain)));
        }
        assert_eq!(
            registry.query(None, None, None, 10, true).unwrap()["rows"][0]["adapter_ids"],
            json!(["adapter.genomics", "adapter.imaging", "adapter.oncology"])
        );
    }

    #[test]
    fn snapshot_round_trip_reverifies_every_bundle() {
        let mut registry = EvidenceBundleRegistry::new();
        registry.import(&bundle("mission-one", "oncology")).unwrap();
        registry.import(&bundle("mission-two", "genomics")).unwrap();
        let snapshot = registry.snapshot().unwrap();
        let restored = EvidenceBundleRegistry::from_snapshot(&snapshot).unwrap();
        assert_eq!(restored.len(), 2);
        assert_eq!(restored.generation(), registry.generation());
    }

    #[test]
    fn tampered_registry_snapshot_is_rejected() {
        let mut registry = EvidenceBundleRegistry::new();
        registry.import(&bundle("mission-one", "oncology")).unwrap();
        let mut snapshot = registry.snapshot().unwrap();
        snapshot["bundles"][0]["bundle"]["catalog_drift"]["status"] = json!("drifted");
        assert!(matches!(
            EvidenceBundleRegistry::from_snapshot(&snapshot),
            Err(EvidenceRegistryError::InvalidSnapshot(_))
        ));
    }
}
