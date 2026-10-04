//! Study-aware cache partitioning for preclinical glioma computation.
//!
//! A shared cache is useful only when reuse rights are proven. This capability partitions local
//! intermediates by study and de-identification scope, permits explicitly public reference assets
//! to cross study boundaries, and refuses protected cross-study reads. It returns a policy-bound
//! decision over handles and hashes; it never opens or exports artifact bytes.

use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P09-F30";
pub const OUTPUT_SCHEMA: &str = "GliomaMultiStudyCachePartitionDecision1@1";
pub const MAX_ITEMS: usize = 4_096;
pub const MAX_TEXT_LEN: usize = 512;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CacheSensitivity {
    PublicReference,
    StudyLocal,
    ProtectedStudy,
    Restricted,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MultiStudyCacheEntry {
    pub entry_id: String,
    pub artifact_id: String,
    pub content_hash: ContentHash,
    pub cache_key_digest: ContentHash,
    pub source_study_id: String,
    pub deidentification_scope: String,
    pub sensitivity: CacheSensitivity,
    pub public_reference: bool,
    pub local_only: bool,
    pub contains_human_data: bool,
    pub contains_direct_identifiers: bool,
    pub contains_clinical_decision: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MultiStudyCachePolicy {
    pub policy_digest: ContentHash,
    pub allowed_scope_order: Vec<String>,
    pub allow_public_reference_reuse: bool,
    pub allow_cross_study_study_local: bool,
    pub require_local_only: bool,
    pub expires_at_tick: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MultiStudyCachePartitionRequest {
    pub requesting_study_id: String,
    pub requested_scope: String,
    pub requested_cache_key_digest: ContentHash,
    pub existing_entries: Vec<MultiStudyCacheEntry>,
    pub policy: MultiStudyCachePolicy,
    pub current_tick: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MultiStudyCachePartitionDisposition {
    SameStudyReuse,
    SharedPublicReference,
    Miss,
    Denied,
    Invalidated,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MultiStudyCachePartitionDecision {
    pub feature_id: String,
    pub output_schema: String,
    pub partition_digest: ContentHash,
    pub requested_study_id: String,
    pub requested_scope: String,
    pub requested_cache_key_digest: ContentHash,
    pub disposition: MultiStudyCachePartitionDisposition,
    pub cache_hit: bool,
    pub reusable_entry: Option<MultiStudyCacheEntry>,
    pub invalidated_entry_order: Vec<String>,
    pub denied_entry_order: Vec<String>,
    pub reason_order: Vec<String>,
    pub negative_evidence: Vec<String>,
    pub uncertainty: Vec<String>,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum MultiStudyCachePartitionError {
    #[error("multi-study cache request is invalid: {0}")]
    InvalidRequest(String),
    #[error("multi-study cache decision is invalid: {0}")]
    InvalidOutput(String),
    #[error("multi-study cache digest failed: {0}")]
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

fn partition_identity(
    request: &MultiStudyCachePartitionRequest,
) -> Result<ContentHash, MultiStudyCachePartitionError> {
    ContentHash::of_value(&serde_json::json!({
        "requesting_study_id": request.requesting_study_id,
        "requested_scope": request.requested_scope,
        "policy_digest": request.policy.policy_digest,
    }))
    .map_err(|error| MultiStudyCachePartitionError::Digest(error.to_string()))
}

fn digest_input(decision: &MultiStudyCachePartitionDecision) -> serde_json::Value {
    serde_json::json!({
        "feature_id": decision.feature_id,
        "output_schema": decision.output_schema,
        "partition_digest": decision.partition_digest,
        "requested_study_id": decision.requested_study_id,
        "requested_scope": decision.requested_scope,
        "requested_cache_key_digest": decision.requested_cache_key_digest,
        "disposition": decision.disposition,
        "cache_hit": decision.cache_hit,
        "reusable_entry": decision.reusable_entry,
        "invalidated_entry_order": decision.invalidated_entry_order,
        "denied_entry_order": decision.denied_entry_order,
        "reason_order": decision.reason_order,
        "negative_evidence": decision.negative_evidence,
        "uncertainty": decision.uncertainty,
    })
}

fn validate_entry(entry: &MultiStudyCacheEntry) -> Result<(), MultiStudyCachePartitionError> {
    if entry.entry_id.trim().is_empty()
        || entry.entry_id.len() > MAX_TEXT_LEN
        || entry.artifact_id.trim().is_empty()
        || entry.artifact_id.len() > MAX_TEXT_LEN
        || !valid_hash(&entry.content_hash)
        || !valid_hash(&entry.cache_key_digest)
        || entry.source_study_id.trim().is_empty()
        || entry.source_study_id.len() > MAX_TEXT_LEN
        || entry.deidentification_scope.trim().is_empty()
        || entry.deidentification_scope.len() > MAX_TEXT_LEN
        || (entry.public_reference && entry.sensitivity != CacheSensitivity::PublicReference)
    {
        return Err(MultiStudyCachePartitionError::InvalidRequest(
            "cache entries require bounded identity, content/key hashes, study/scope labels, and consistent public-reference sensitivity".into(),
        ));
    }
    if entry.contains_human_data
        || entry.contains_direct_identifiers
        || entry.contains_clinical_decision
    {
        return Err(MultiStudyCachePartitionError::InvalidRequest(
            "human, direct-identifier, and clinical-decision cache entries are outside the preclinical engine boundary".into(),
        ));
    }
    Ok(())
}

fn validate_request(
    request: &MultiStudyCachePartitionRequest,
) -> Result<(), MultiStudyCachePartitionError> {
    if request.requesting_study_id.trim().is_empty()
        || request.requesting_study_id.len() > MAX_TEXT_LEN
        || request.requested_scope.trim().is_empty()
        || request.requested_scope.len() > MAX_TEXT_LEN
        || !valid_hash(&request.requested_cache_key_digest)
        || request.current_tick == 0
        || request.policy.expires_at_tick == 0
        || request.current_tick > request.policy.expires_at_tick
        || !valid_hash(&request.policy.policy_digest)
        || !unique_bounded(&request.policy.allowed_scope_order, MAX_ITEMS)
        || !canonical(&request.policy.allowed_scope_order)
        || !request
            .policy
            .allowed_scope_order
            .iter()
            .any(|scope| scope == &request.requested_scope)
        || request.existing_entries.len() > MAX_ITEMS
    {
        return Err(MultiStudyCachePartitionError::InvalidRequest(
            "live study-aware cache policy, canonical permitted scopes, and content-addressed request identity are required".into(),
        ));
    }
    let mut ids = BTreeSet::new();
    for entry in &request.existing_entries {
        validate_entry(entry)?;
        if !ids.insert(entry.entry_id.clone()) {
            return Err(MultiStudyCachePartitionError::InvalidRequest(
                "cache entry identities must be unique".into(),
            ));
        }
    }
    Ok(())
}

impl MultiStudyCachePartitionDecision {
    pub fn validate(&self) -> Result<(), MultiStudyCachePartitionError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || !valid_hash(&self.partition_digest)
            || self.requested_study_id.trim().is_empty()
            || self.requested_scope.trim().is_empty()
            || !valid_hash(&self.requested_cache_key_digest)
            || !unique_bounded(&self.invalidated_entry_order, MAX_ITEMS)
            || !canonical(&self.invalidated_entry_order)
            || !unique_bounded(&self.denied_entry_order, MAX_ITEMS)
            || !canonical(&self.denied_entry_order)
            || !unique_bounded(&self.reason_order, MAX_ITEMS)
            || !canonical(&self.reason_order)
            || !unique_bounded(&self.negative_evidence, MAX_ITEMS)
            || !canonical(&self.negative_evidence)
            || !unique_bounded(&self.uncertainty, MAX_ITEMS)
            || !canonical(&self.uncertainty)
            || (self.cache_hit != self.reusable_entry.is_some())
            || !valid_hash(&self.digest)
        {
            return Err(MultiStudyCachePartitionError::InvalidOutput(
                "partition identity, ordered isolation outcomes, cache hit binding, or digest is invalid".into(),
            ));
        }
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| MultiStudyCachePartitionError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(MultiStudyCachePartitionError::InvalidOutput(
                "partition decision digest is not content-addressed".into(),
            ));
        }
        Ok(())
    }
}

/// Decide whether a local cache object may be reused by the requesting study and scope.
pub fn partition_glioma_multistudy_cache(
    request: &MultiStudyCachePartitionRequest,
) -> Result<MultiStudyCachePartitionDecision, MultiStudyCachePartitionError> {
    validate_request(request)?;
    let partition_digest = partition_identity(request)?;
    let entries = request
        .existing_entries
        .iter()
        .cloned()
        .map(|entry| (entry.entry_id.clone(), entry))
        .collect::<BTreeMap<_, _>>();
    let mut invalidated = BTreeSet::new();
    let mut denied = BTreeSet::new();
    let mut reasons = BTreeSet::new();
    let mut reusable_entry = None;
    let mut disposition = MultiStudyCachePartitionDisposition::Miss;
    for entry in entries.values() {
        if entry.cache_key_digest != request.requested_cache_key_digest {
            invalidated.insert(entry.entry_id.clone());
            reasons.insert("cache-key-or-policy-identity-changed".into());
            continue;
        }
        if request.policy.require_local_only && !entry.local_only {
            denied.insert(entry.entry_id.clone());
            reasons.insert("locality-policy-denied-cross-host-entry".into());
            continue;
        }
        let same_study = entry.source_study_id == request.requesting_study_id
            && entry.deidentification_scope == request.requested_scope;
        let public_cross_study = entry.source_study_id != request.requesting_study_id
            && entry.public_reference
            && entry.sensitivity == CacheSensitivity::PublicReference
            && request.policy.allow_public_reference_reuse;
        let study_local_cross_study = entry.source_study_id != request.requesting_study_id
            && entry.sensitivity == CacheSensitivity::StudyLocal
            && request.policy.allow_cross_study_study_local;
        if same_study || public_cross_study || study_local_cross_study {
            if reusable_entry.is_none() {
                reusable_entry = Some(entry.clone());
                disposition = if same_study {
                    MultiStudyCachePartitionDisposition::SameStudyReuse
                } else {
                    MultiStudyCachePartitionDisposition::SharedPublicReference
                };
            } else {
                denied.insert(entry.entry_id.clone());
                reasons.insert("multiple_matching_entries-reuse-is-ambiguous".into());
            }
        } else {
            denied.insert(entry.entry_id.clone());
            reasons.insert("protected-cross-study-read-denied".into());
        }
    }
    if reusable_entry.is_none() && !invalidated.is_empty() {
        disposition = MultiStudyCachePartitionDisposition::Invalidated;
    } else if reusable_entry.is_none() && !denied.is_empty() {
        disposition = MultiStudyCachePartitionDisposition::Denied;
    }
    let mut negative_evidence = Vec::new();
    let mut uncertainty = Vec::new();
    if !denied.is_empty() {
        negative_evidence.push("cross-study-or-locality-cache-read-denied".into());
    }
    if !invalidated.is_empty() {
        negative_evidence.push("cache-policy-or-key-change-invalidated-entry".into());
    }
    if reusable_entry.is_none() {
        uncertainty.push("no-authorized-cache-entry-for-requested-study-and-scope".into());
    }
    negative_evidence.sort();
    uncertainty.sort();
    let mut decision = MultiStudyCachePartitionDecision {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        partition_digest,
        requested_study_id: request.requesting_study_id.clone(),
        requested_scope: request.requested_scope.clone(),
        requested_cache_key_digest: request.requested_cache_key_digest.clone(),
        disposition,
        cache_hit: reusable_entry.is_some(),
        reusable_entry,
        invalidated_entry_order: invalidated.into_iter().collect(),
        denied_entry_order: denied.into_iter().collect(),
        reason_order: reasons.into_iter().collect(),
        negative_evidence,
        uncertainty,
        digest: ContentHash::of_bytes(b"unsealed-glioma-multistudy-cache-partition"),
    };
    decision.digest = ContentHash::of_value(&digest_input(&decision))
        .map_err(|error| MultiStudyCachePartitionError::Digest(error.to_string()))?;
    decision.validate()?;
    Ok(decision)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hash(value: &str) -> ContentHash {
        ContentHash::of_bytes(value.as_bytes())
    }

    fn entry(
        id: &str,
        study: &str,
        scope: &str,
        sensitivity: CacheSensitivity,
        public_reference: bool,
    ) -> MultiStudyCacheEntry {
        MultiStudyCacheEntry {
            entry_id: id.into(),
            artifact_id: format!("artifact-{id}"),
            content_hash: hash(id),
            cache_key_digest: hash("key"),
            source_study_id: study.into(),
            deidentification_scope: scope.into(),
            sensitivity,
            public_reference,
            local_only: true,
            contains_human_data: false,
            contains_direct_identifiers: false,
            contains_clinical_decision: false,
        }
    }

    fn request(entries: Vec<MultiStudyCacheEntry>) -> MultiStudyCachePartitionRequest {
        MultiStudyCachePartitionRequest {
            requesting_study_id: "study-b".into(),
            requested_scope: "deidentified-organoid".into(),
            requested_cache_key_digest: hash("key"),
            existing_entries: entries,
            policy: MultiStudyCachePolicy {
                policy_digest: hash("policy"),
                allowed_scope_order: vec!["deidentified-organoid".into()],
                allow_public_reference_reuse: true,
                allow_cross_study_study_local: false,
                require_local_only: true,
                expires_at_tick: 100,
            },
            current_tick: 1,
        }
    }

    #[test]
    fn same_study_scope_reuses_entry() {
        let decision = partition_glioma_multistudy_cache(&request(vec![entry(
            "same",
            "study-b",
            "deidentified-organoid",
            CacheSensitivity::StudyLocal,
            false,
        )]))
        .expect("decision");
        assert_eq!(
            decision.disposition,
            MultiStudyCachePartitionDisposition::SameStudyReuse
        );
        assert!(decision.cache_hit);
    }

    #[test]
    fn public_reference_can_cross_study_boundary() {
        let decision = partition_glioma_multistudy_cache(&request(vec![entry(
            "public",
            "study-a",
            "public-reference",
            CacheSensitivity::PublicReference,
            true,
        )]))
        .expect("decision");
        assert_eq!(
            decision.disposition,
            MultiStudyCachePartitionDisposition::SharedPublicReference
        );
    }

    #[test]
    fn protected_cross_study_entry_is_denied() {
        let decision = partition_glioma_multistudy_cache(&request(vec![entry(
            "protected",
            "study-a",
            "deidentified-organoid",
            CacheSensitivity::ProtectedStudy,
            false,
        )]))
        .expect("decision");
        assert_eq!(
            decision.disposition,
            MultiStudyCachePartitionDisposition::Denied
        );
        assert!(!decision.cache_hit);
        assert!(decision
            .negative_evidence
            .iter()
            .any(|item| item.contains("denied")));
    }

    #[test]
    fn changed_cache_key_is_invalidated() {
        let mut item = entry(
            "changed",
            "study-a",
            "public-reference",
            CacheSensitivity::PublicReference,
            true,
        );
        item.cache_key_digest = hash("old-key");
        let decision = partition_glioma_multistudy_cache(&request(vec![item])).expect("decision");
        assert_eq!(
            decision.disposition,
            MultiStudyCachePartitionDisposition::Invalidated
        );
        assert_eq!(decision.invalidated_entry_order, vec!["changed"]);
    }

    #[test]
    fn protected_boundary_flags_fail_closed() {
        let mut item = entry(
            "human",
            "study-a",
            "public-reference",
            CacheSensitivity::PublicReference,
            true,
        );
        item.contains_human_data = true;
        assert!(matches!(
            partition_glioma_multistudy_cache(&request(vec![item])),
            Err(MultiStudyCachePartitionError::InvalidRequest(_))
        ));
    }
}
