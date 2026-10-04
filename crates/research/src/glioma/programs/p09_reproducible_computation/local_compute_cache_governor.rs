//! Deterministic local computation-cache governance for preclinical glioma workflows.
//!
//! The governor is stronger than a task-level cache lookup: it binds reuse to inputs, code,
//! environment, policy, semantic version, and output schema, then applies an explicit retention
//! and quota policy. It may evict only unpinned intermediates and never treats a changed policy or
//! dependency as a cache hit. Artifacts remain local handles; this module never reads raw bytes.

use crate::glioma_engine::LocalArtifactRef;
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P09-F29";
pub const OUTPUT_SCHEMA: &str = "GliomaComputeCacheDecision1@1";
pub const MAX_ENTRIES: usize = 4_096;
pub const MAX_INPUTS: usize = 256;
pub const MAX_TEXT_LEN: usize = 512;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CacheKey {
    pub task_id: String,
    pub input_hash_order: Vec<ContentHash>,
    pub code_digest: ContentHash,
    pub environment_digest: ContentHash,
    pub policy_digest: ContentHash,
    pub semantic_version: String,
    pub output_schema: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CacheEntry {
    pub entry_id: String,
    pub key: CacheKey,
    pub artifact: LocalArtifactRef,
    pub created_tick: u64,
    pub last_used_tick: u64,
    pub size_units: u64,
    pub pinned: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CacheEvictionPolicy {
    LeastRecentlyUsed,
    OldestFirst,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CacheRetentionPolicy {
    pub allow_reuse: bool,
    pub require_local_only: bool,
    pub max_entries: usize,
    pub max_total_size_units: u64,
    pub retention_ticks: u64,
    pub eviction_policy: CacheEvictionPolicy,
    pub policy_digest: ContentHash,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ComputeCacheGovernorRequest {
    pub key: CacheKey,
    pub existing_entries: Vec<CacheEntry>,
    pub incoming_entry: Option<CacheEntry>,
    pub policy: CacheRetentionPolicy,
    pub current_tick: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ComputeCacheDecisionDisposition {
    Hit,
    MissAdmitted,
    MissNotAdmitted,
    Disabled,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ComputeCacheDecision {
    pub feature_id: String,
    pub output_schema: String,
    pub key_digest: ContentHash,
    pub disposition: ComputeCacheDecisionDisposition,
    pub cache_hit: bool,
    pub reusable_artifact: Option<LocalArtifactRef>,
    pub retained_entry_order: Vec<String>,
    pub evicted_entry_order: Vec<String>,
    pub invalid_entry_order: Vec<String>,
    pub reason_order: Vec<String>,
    pub negative_evidence: Vec<String>,
    pub uncertainty: Vec<String>,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ComputeCacheGovernorError {
    #[error("compute-cache request is invalid: {0}")]
    InvalidRequest(String),
    #[error("compute-cache decision is invalid: {0}")]
    InvalidOutput(String),
    #[error("compute-cache digest failed: {0}")]
    Digest(String),
}

fn valid_hash(value: &ContentHash) -> bool {
    value.as_str().len() == 64
}

fn canonical(values: &[String]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn unique_bounded(values: &[String], max: usize) -> bool {
    let mut seen = BTreeSet::new();
    values.len() <= max
        && values.iter().all(|value| {
            !value.trim().is_empty() && value.len() <= MAX_TEXT_LEN && seen.insert(value)
        })
}

fn key_digest(key: &CacheKey) -> Result<ContentHash, ComputeCacheGovernorError> {
    let value = serde_json::to_value(key)
        .map_err(|error| ComputeCacheGovernorError::Digest(error.to_string()))?;
    ContentHash::of_value(&value)
        .map_err(|error| ComputeCacheGovernorError::Digest(error.to_string()))
}

fn digest_input(decision: &ComputeCacheDecision) -> serde_json::Value {
    serde_json::json!({
        "feature_id": decision.feature_id,
        "output_schema": decision.output_schema,
        "key_digest": decision.key_digest,
        "disposition": decision.disposition,
        "cache_hit": decision.cache_hit,
        "reusable_artifact": decision.reusable_artifact,
        "retained_entry_order": decision.retained_entry_order,
        "evicted_entry_order": decision.evicted_entry_order,
        "invalid_entry_order": decision.invalid_entry_order,
        "reason_order": decision.reason_order,
        "negative_evidence": decision.negative_evidence,
        "uncertainty": decision.uncertainty,
    })
}

fn validate_key(key: &CacheKey) -> Result<(), ComputeCacheGovernorError> {
    if key.task_id.trim().is_empty()
        || key.task_id.len() > MAX_TEXT_LEN
        || key.input_hash_order.is_empty()
        || key.input_hash_order.len() > MAX_INPUTS
        || key.input_hash_order.iter().any(|hash| !valid_hash(hash))
        || !valid_hash(&key.code_digest)
        || !valid_hash(&key.environment_digest)
        || !valid_hash(&key.policy_digest)
        || key.semantic_version.trim().is_empty()
        || key.semantic_version.len() > MAX_TEXT_LEN
        || key.output_schema.trim().is_empty()
        || key.output_schema.len() > MAX_TEXT_LEN
    {
        return Err(ComputeCacheGovernorError::InvalidRequest(
            "cache keys require bounded task, input, code, environment, policy, version, and output identities".into(),
        ));
    }
    Ok(())
}

fn validate_entry(
    entry: &CacheEntry,
    policy: &CacheRetentionPolicy,
) -> Result<(), ComputeCacheGovernorError> {
    if entry.entry_id.trim().is_empty()
        || entry.entry_id.len() > MAX_TEXT_LEN
        || entry.created_tick == 0
        || entry.last_used_tick < entry.created_tick
        || entry.size_units == 0
        || entry.size_units > policy.max_total_size_units
    {
        return Err(ComputeCacheGovernorError::InvalidRequest(
            "cache entries require bounded identity, monotonic ticks, and positive size within quota".into(),
        ));
    }
    validate_key(&entry.key)?;
    entry.artifact.validate().map_err(|error| {
        ComputeCacheGovernorError::InvalidRequest(format!("cache artifact is invalid: {error}"))
    })?;
    if entry.artifact.content_type != entry.key.output_schema {
        return Err(ComputeCacheGovernorError::InvalidRequest(
            "cache artifact content type must match the key output schema".into(),
        ));
    }
    Ok(())
}

fn validate_request(
    request: &ComputeCacheGovernorRequest,
) -> Result<(), ComputeCacheGovernorError> {
    validate_key(&request.key)?;
    let policy = &request.policy;
    if request.current_tick == 0
        || policy.max_entries == 0
        || policy.max_entries > MAX_ENTRIES
        || policy.max_total_size_units == 0
        || policy.retention_ticks == 0
        || !valid_hash(&policy.policy_digest)
        || request.existing_entries.len() > MAX_ENTRIES
    {
        return Err(ComputeCacheGovernorError::InvalidRequest(
            "cache policy requires positive bounded entry, size, retention, and policy identity limits".into(),
        ));
    }
    let mut ids = BTreeSet::new();
    for entry in &request.existing_entries {
        validate_entry(entry, policy)?;
        if !ids.insert(entry.entry_id.clone()) {
            return Err(ComputeCacheGovernorError::InvalidRequest(
                "cache entry identities must be unique".into(),
            ));
        }
    }
    if let Some(entry) = &request.incoming_entry {
        validate_entry(entry, policy)?;
        if entry.key != request.key {
            return Err(ComputeCacheGovernorError::InvalidRequest(
                "incoming cache entry must bind to the requested key".into(),
            ));
        }
        if ids.contains(&entry.entry_id) {
            return Err(ComputeCacheGovernorError::InvalidRequest(
                "incoming cache entry identity already exists".into(),
            ));
        }
    }
    Ok(())
}

impl ComputeCacheDecision {
    pub fn validate(&self) -> Result<(), ComputeCacheGovernorError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || !valid_hash(&self.key_digest)
            || !unique_bounded(&self.retained_entry_order, MAX_ENTRIES)
            || !canonical(&self.retained_entry_order)
            || !unique_bounded(&self.evicted_entry_order, MAX_ENTRIES)
            || !canonical(&self.evicted_entry_order)
            || !unique_bounded(&self.invalid_entry_order, MAX_ENTRIES)
            || !canonical(&self.invalid_entry_order)
            || !unique_bounded(&self.reason_order, MAX_ENTRIES)
            || !canonical(&self.reason_order)
            || !unique_bounded(&self.negative_evidence, MAX_ENTRIES)
            || !canonical(&self.negative_evidence)
            || !unique_bounded(&self.uncertainty, MAX_ENTRIES)
            || !canonical(&self.uncertainty)
            || (self.cache_hit != self.reusable_artifact.is_some())
            || !valid_hash(&self.digest)
        {
            return Err(ComputeCacheGovernorError::InvalidOutput(
                "cache decision identity, ordered entry outcomes, hit/artifact binding, or digest is invalid".into(),
            ));
        }
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| ComputeCacheGovernorError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(ComputeCacheGovernorError::InvalidOutput(
                "cache decision digest is not content-addressed".into(),
            ));
        }
        Ok(())
    }
}

/// Decide whether a deterministic local intermediate can be reused or admitted under quota.
pub fn govern_glioma_compute_cache(
    request: &ComputeCacheGovernorRequest,
) -> Result<ComputeCacheDecision, ComputeCacheGovernorError> {
    validate_request(request)?;
    let requested_digest = key_digest(&request.key)?;
    let mut entries = request
        .existing_entries
        .iter()
        .cloned()
        .map(|entry| (entry.entry_id.clone(), entry))
        .collect::<BTreeMap<_, _>>();
    let mut invalid = BTreeSet::new();
    let mut reasons = BTreeSet::new();
    let mut reusable_artifact = None;
    let mut cache_hit = false;
    if request.policy.allow_reuse {
        for entry in entries.values() {
            if entry.key == request.key
                && request.current_tick.saturating_sub(entry.last_used_tick)
                    <= request.policy.retention_ticks
                && (!request.policy.require_local_only || entry.artifact.local_only)
            {
                if reusable_artifact.is_none() {
                    reusable_artifact = Some(entry.artifact.clone());
                    cache_hit = true;
                } else {
                    invalid.insert(entry.entry_id.clone());
                    reasons.insert("duplicate-matching-cache-entry-invalidated".into());
                }
            } else if entry.key != request.key {
                invalid.insert(entry.entry_id.clone());
                reasons.insert(
                    "dependency-code-environment-policy-or-version-identity-changed".into(),
                );
            } else if request.current_tick.saturating_sub(entry.last_used_tick)
                > request.policy.retention_ticks
            {
                invalid.insert(entry.entry_id.clone());
                reasons.insert("cache-entry-exceeded-retention-window".into());
            } else if request.policy.require_local_only && !entry.artifact.local_only {
                invalid.insert(entry.entry_id.clone());
                reasons.insert("locality-policy-invalidated-cache-entry".into());
            }
        }
    } else {
        reasons.insert("cache-reuse-disabled-by-policy".into());
    }
    let mut evicted = BTreeSet::new();
    let mut negative_evidence = Vec::new();
    let mut uncertainty = Vec::new();
    if !request.policy.allow_reuse {
        negative_evidence.push("policy-disallows-cache-reuse".into());
    }
    if cache_hit {
        let hit_id = entries
            .values()
            .find(|entry| {
                entry.key == request.key && entry.artifact == reusable_artifact.clone().unwrap()
            })
            .map(|entry| entry.entry_id.clone());
        if let Some(hit_id) = hit_id {
            if let Some(entry) = entries.get_mut(&hit_id) {
                entry.last_used_tick = request.current_tick;
            }
        }
    }
    let mut disposition = if cache_hit {
        ComputeCacheDecisionDisposition::Hit
    } else if !request.policy.allow_reuse {
        ComputeCacheDecisionDisposition::Disabled
    } else {
        ComputeCacheDecisionDisposition::MissNotAdmitted
    };
    if !cache_hit {
        if let Some(incoming) = &request.incoming_entry {
            entries.insert(incoming.entry_id.clone(), incoming.clone());
            let mut total_size = entries.values().map(|entry| entry.size_units).sum::<u64>();
            let mut eviction_candidates = entries
                .values()
                .filter(|entry| !entry.pinned && entry.entry_id != incoming.entry_id)
                .cloned()
                .collect::<Vec<_>>();
            eviction_candidates.sort_by(|left, right| {
                let left_key = match request.policy.eviction_policy {
                    CacheEvictionPolicy::LeastRecentlyUsed => left.last_used_tick,
                    CacheEvictionPolicy::OldestFirst => left.created_tick,
                };
                let right_key = match request.policy.eviction_policy {
                    CacheEvictionPolicy::LeastRecentlyUsed => right.last_used_tick,
                    CacheEvictionPolicy::OldestFirst => right.created_tick,
                };
                left_key
                    .cmp(&right_key)
                    .then(left.entry_id.cmp(&right.entry_id))
            });
            while (entries.len() > request.policy.max_entries
                || total_size > request.policy.max_total_size_units)
                && !eviction_candidates.is_empty()
            {
                let candidate = eviction_candidates.remove(0);
                total_size = total_size.saturating_sub(candidate.size_units);
                entries.remove(&candidate.entry_id);
                evicted.insert(candidate.entry_id);
            }
            if entries.len() <= request.policy.max_entries
                && total_size <= request.policy.max_total_size_units
            {
                disposition = ComputeCacheDecisionDisposition::MissAdmitted;
            } else {
                disposition = ComputeCacheDecisionDisposition::Blocked;
                negative_evidence.push("pinned-or-incoming-cache-entries-exceed-quota".into());
            }
        } else {
            uncertainty.push("cache-miss-has-no-incoming-entry-to-admit".into());
        }
    }
    let mut retained = entries.keys().cloned().collect::<Vec<_>>();
    retained.sort();
    let mut invalid_order = invalid.into_iter().collect::<Vec<_>>();
    invalid_order.sort();
    let mut reason_order = reasons.into_iter().collect::<Vec<_>>();
    reason_order.sort();
    negative_evidence.sort();
    uncertainty.sort();
    let mut decision = ComputeCacheDecision {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        key_digest: requested_digest,
        disposition,
        cache_hit,
        reusable_artifact,
        retained_entry_order: retained,
        evicted_entry_order: evicted.into_iter().collect(),
        invalid_entry_order: invalid_order,
        reason_order,
        negative_evidence,
        uncertainty,
        digest: ContentHash::of_bytes(b"unsealed-glioma-cache-decision"),
    };
    decision.digest = ContentHash::of_value(&digest_input(&decision))
        .map_err(|error| ComputeCacheGovernorError::Digest(error.to_string()))?;
    decision.validate()?;
    Ok(decision)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hash(value: &str) -> ContentHash {
        ContentHash::of_bytes(value.as_bytes())
    }

    fn key() -> CacheKey {
        CacheKey {
            task_id: "normalize".into(),
            input_hash_order: vec![hash("input")],
            code_digest: hash("code"),
            environment_digest: hash("environment"),
            policy_digest: hash("policy"),
            semantic_version: "1.0.0".into(),
            output_schema: "normalized-image".into(),
        }
    }

    fn entry(id: &str, cache_key: CacheKey, tick: u64, size: u64, pinned: bool) -> CacheEntry {
        CacheEntry {
            entry_id: id.into(),
            key: cache_key,
            artifact: LocalArtifactRef {
                artifact_id: format!("artifact-{id}"),
                content_hash: hash(id),
                content_type: "normalized-image".into(),
                local_only: true,
                contains_human_data: false,
                contains_direct_identifiers: false,
            },
            created_tick: tick,
            last_used_tick: tick,
            size_units: size,
            pinned,
        }
    }

    fn request(
        existing_entries: Vec<CacheEntry>,
        incoming_entry: Option<CacheEntry>,
    ) -> ComputeCacheGovernorRequest {
        ComputeCacheGovernorRequest {
            key: key(),
            existing_entries,
            incoming_entry,
            policy: CacheRetentionPolicy {
                allow_reuse: true,
                require_local_only: true,
                max_entries: 2,
                max_total_size_units: 10,
                retention_ticks: 10,
                eviction_policy: CacheEvictionPolicy::LeastRecentlyUsed,
                policy_digest: hash("policy"),
            },
            current_tick: 5,
        }
    }

    #[test]
    fn exact_key_reuses_local_artifact() {
        let decision =
            govern_glioma_compute_cache(&request(vec![entry("hit", key(), 1, 2, false)], None))
                .expect("decision");
        assert_eq!(decision.disposition, ComputeCacheDecisionDisposition::Hit);
        assert!(decision.cache_hit);
        assert!(decision.reusable_artifact.is_some());
    }

    #[test]
    fn changed_dependency_identity_invalidates_reuse() {
        let mut changed = key();
        changed.environment_digest = hash("new-environment");
        let decision =
            govern_glioma_compute_cache(&request(vec![entry("old", changed, 1, 2, false)], None))
                .expect("decision");
        assert_eq!(
            decision.disposition,
            ComputeCacheDecisionDisposition::MissNotAdmitted
        );
        assert!(!decision.cache_hit);
        assert!(decision
            .reason_order
            .iter()
            .any(|reason| reason.contains("identity")));
    }

    #[test]
    fn quota_evicts_old_unpinned_entry_and_admits_incoming() {
        let incoming = entry("incoming", key(), 5, 6, false);
        let mut old_key = key();
        old_key.environment_digest = hash("old-environment");
        let decision = govern_glioma_compute_cache(&request(
            vec![entry("old", old_key, 1, 5, false)],
            Some(incoming),
        ))
        .expect("decision");
        assert_eq!(
            decision.disposition,
            ComputeCacheDecisionDisposition::MissAdmitted
        );
        assert_eq!(decision.evicted_entry_order, vec!["old"]);
        assert!(decision.retained_entry_order.contains(&"incoming".into()));
    }

    #[test]
    fn pinned_entries_block_quota_overflow() {
        let incoming = entry("incoming", key(), 5, 6, false);
        let mut pinned_key = key();
        pinned_key.environment_digest = hash("pinned-environment");
        let decision = govern_glioma_compute_cache(&request(
            vec![entry("pinned", pinned_key, 1, 5, true)],
            Some(incoming),
        ))
        .expect("decision");
        assert_eq!(
            decision.disposition,
            ComputeCacheDecisionDisposition::Blocked
        );
        assert!(decision
            .negative_evidence
            .iter()
            .any(|item| item.contains("quota")));
    }

    #[test]
    fn disabled_policy_never_reports_a_hit() {
        let mut req = request(vec![entry("hit", key(), 1, 2, false)], None);
        req.policy.allow_reuse = false;
        let decision = govern_glioma_compute_cache(&req).expect("decision");
        assert_eq!(
            decision.disposition,
            ComputeCacheDecisionDisposition::Disabled
        );
        assert!(!decision.cache_hit);
    }
}
