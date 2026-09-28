//! Verified, locality-aware archival mirror assessment for preclinical glioma research objects.
//!
//! The mirror planner compares signed source digests with policy-approved replicas and emits
//! repair work without copying bytes itself. Missing, stale, corrupt, unavailable, or
//! out-of-region replicas are distinct outcomes so an operations team can recover a research
//! object without silently creating an unauthorized copy.

use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P11-F30";
pub const OUTPUT_SCHEMA: &str = "GliomaDistributedArchiveMirror1@1";
pub const MAX_VERSIONS: usize = 2_048;
pub const MAX_REPLICAS: usize = 16_384;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MirrorSourceVersion {
    pub version_id: String,
    pub content_digest: ContentHash,
    pub approved_region_order: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArchiveReplica {
    pub replica_id: String,
    pub version_id: String,
    pub region: String,
    pub content_digest: ContentHash,
    pub verified_epoch: u64,
    pub available: bool,
    pub policy_allowed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArchiveMirrorPolicy {
    pub now_epoch: u64,
    pub stale_after_epochs: u64,
    pub required_replica_count: u16,
    pub require_locality: bool,
    pub max_repair_tasks: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DistributedMirrorRequest {
    pub sources: Vec<MirrorSourceVersion>,
    pub replicas: Vec<ArchiveReplica>,
    pub policy: ArchiveMirrorPolicy,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MirrorStatus {
    Healthy,
    RepairRequired,
    Blocked,
    Unresolved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MirrorVersionStatus {
    pub version_id: String,
    pub status: MirrorStatus,
    pub verified_replica_order: Vec<String>,
    pub repair_replica_order: Vec<String>,
    pub finding_order: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MirrorDisposition {
    Synchronized,
    Partial,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReplicaSetStatus {
    pub feature_id: String,
    pub output_schema: String,
    pub versions: Vec<MirrorVersionStatus>,
    pub repair_order: Vec<String>,
    pub blocked_order: Vec<String>,
    pub unresolved_order: Vec<String>,
    pub disposition: MirrorDisposition,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum DistributedMirrorError {
    #[error("distributed mirror request is invalid: {0}")]
    InvalidRequest(String),
    #[error("distributed mirror output is invalid: {0}")]
    InvalidOutput(String),
    #[error("distributed mirror digest failed: {0}")]
    Digest(String),
}

fn valid_id(value: &str) -> bool {
    !value.trim().is_empty()
        && value.len() <= 160
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"._:/-@".contains(&byte))
}
fn valid_digest(value: &ContentHash) -> bool {
    value.as_str().len() == 64
}
fn digest_input(status: &ReplicaSetStatus) -> serde_json::Value {
    serde_json::json!({"feature_id":status.feature_id,"output_schema":status.output_schema,"versions":status.versions,"repair_order":status.repair_order,"blocked_order":status.blocked_order,"unresolved_order":status.unresolved_order,"disposition":status.disposition})
}
fn validate_request(request: &DistributedMirrorRequest) -> Result<(), DistributedMirrorError> {
    if request.sources.is_empty()
        || request.sources.len() > MAX_VERSIONS
        || request.replicas.len() > MAX_REPLICAS
        || request.policy.now_epoch == 0
        || request.policy.required_replica_count == 0
        || request.policy.max_repair_tasks == 0
        || request.sources.iter().any(|source| {
            !valid_id(&source.version_id)
                || !valid_digest(&source.content_digest)
                || source
                    .approved_region_order
                    .windows(2)
                    .any(|pair| pair[0] >= pair[1])
                || source
                    .approved_region_order
                    .iter()
                    .any(|region| !valid_id(region))
        })
        || request.replicas.iter().any(|replica| {
            !valid_id(&replica.replica_id)
                || !valid_id(&replica.version_id)
                || !valid_id(&replica.region)
                || !valid_digest(&replica.content_digest)
                || replica.verified_epoch > request.policy.now_epoch
        })
    {
        return Err(DistributedMirrorError::InvalidRequest(
            "bounded source, replica, epoch, digest, and canonical locality fields are required"
                .into(),
        ));
    }
    let mut ids = BTreeSet::new();
    if request
        .sources
        .iter()
        .any(|source| !ids.insert(source.version_id.clone()))
    {
        return Err(DistributedMirrorError::InvalidRequest(
            "source version identifiers must be unique".into(),
        ));
    }
    Ok(())
}
impl ReplicaSetStatus {
    pub fn validate(&self) -> Result<(), DistributedMirrorError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || self
                .versions
                .windows(2)
                .any(|pair| pair[0].version_id >= pair[1].version_id)
        {
            return Err(DistributedMirrorError::InvalidOutput(
                "mirror identity or ordering is invalid".into(),
            ));
        }
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| DistributedMirrorError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(DistributedMirrorError::InvalidOutput(
                "mirror status digest is not content-addressed".into(),
            ));
        }
        Ok(())
    }
}

/// Assess archival replicas and emit a bounded repair plan without moving any bytes.
pub fn evaluate_glioma_distributed_archive_mirror(
    request: &DistributedMirrorRequest,
) -> Result<ReplicaSetStatus, DistributedMirrorError> {
    validate_request(request)?;
    let mut statuses = Vec::new();
    let mut repair = Vec::new();
    let mut blocked = Vec::new();
    let mut unresolved = Vec::new();
    for source in &request.sources {
        let replicas = request
            .replicas
            .iter()
            .filter(|replica| replica.version_id == source.version_id)
            .collect::<Vec<_>>();
        let mut verified = Vec::new();
        let mut repair_ids = Vec::new();
        let mut findings = BTreeSet::new();
        for replica in &replicas {
            if !replica.policy_allowed
                || (request.policy.require_locality
                    && !source.approved_region_order.contains(&replica.region))
            {
                findings.insert(format!(
                    "{}:unauthorized-region-or-policy",
                    replica.replica_id
                ));
                blocked.push(replica.replica_id.clone());
            } else if !replica.available {
                findings.insert(format!("{}:unavailable", replica.replica_id));
                repair_ids.push(replica.replica_id.clone());
            } else if replica.content_digest != source.content_digest {
                findings.insert(format!("{}:digest-mismatch", replica.replica_id));
                repair_ids.push(replica.replica_id.clone());
            } else if request
                .policy
                .now_epoch
                .saturating_sub(replica.verified_epoch)
                > request.policy.stale_after_epochs
            {
                findings.insert(format!("{}:stale-verification", replica.replica_id));
                repair_ids.push(replica.replica_id.clone());
            } else {
                verified.push(replica.replica_id.clone());
            }
        }
        verified.sort();
        repair_ids.sort();
        let mut status = if verified.len() >= request.policy.required_replica_count as usize {
            MirrorStatus::Healthy
        } else if replicas.is_empty() {
            findings.insert("missing-replica".into());
            MirrorStatus::Unresolved
        } else if findings
            .iter()
            .any(|finding| finding.contains("unauthorized"))
        {
            MirrorStatus::Blocked
        } else {
            MirrorStatus::RepairRequired
        };
        if repair.len() + repair_ids.len() > request.policy.max_repair_tasks {
            findings.insert("repair-budget-exhausted".into());
            status = MirrorStatus::Blocked;
            repair_ids.clear();
        }
        repair.extend(repair_ids.iter().cloned());
        unresolved.extend(if matches!(status, MirrorStatus::Unresolved) {
            vec![source.version_id.clone()]
        } else {
            Vec::new()
        });
        blocked.extend(if matches!(status, MirrorStatus::Blocked) {
            vec![source.version_id.clone()]
        } else {
            Vec::new()
        });
        statuses.push(MirrorVersionStatus {
            version_id: source.version_id.clone(),
            status,
            verified_replica_order: verified,
            repair_replica_order: repair_ids,
            finding_order: findings.into_iter().collect(),
        });
    }
    statuses.sort_by(|left, right| left.version_id.cmp(&right.version_id));
    repair.sort();
    repair.dedup();
    blocked.sort();
    blocked.dedup();
    unresolved.sort();
    unresolved.dedup();
    let disposition = if !blocked.is_empty() || !unresolved.is_empty() {
        MirrorDisposition::Blocked
    } else if !repair.is_empty() {
        MirrorDisposition::Partial
    } else {
        MirrorDisposition::Synchronized
    };
    let mut status = ReplicaSetStatus {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        versions: statuses,
        repair_order: repair,
        blocked_order: blocked,
        unresolved_order: unresolved,
        disposition,
        digest: ContentHash::of_bytes(b"unsealed-glioma-mirror-status"),
    };
    status.digest = ContentHash::of_value(&digest_input(&status))
        .map_err(|error| DistributedMirrorError::Digest(error.to_string()))?;
    status.validate()?;
    Ok(status)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn hash(label: &str) -> ContentHash {
        ContentHash::of_value(&serde_json::json!({"label": label})).unwrap()
    }
    fn request() -> DistributedMirrorRequest {
        DistributedMirrorRequest {
            sources: vec![MirrorSourceVersion {
                version_id: "v1".into(),
                content_digest: hash("v1"),
                approved_region_order: vec!["eu".into(), "us".into()],
            }],
            replicas: vec![
                ArchiveReplica {
                    replica_id: "r1".into(),
                    version_id: "v1".into(),
                    region: "eu".into(),
                    content_digest: hash("v1"),
                    verified_epoch: 100,
                    available: true,
                    policy_allowed: true,
                },
                ArchiveReplica {
                    replica_id: "r2".into(),
                    version_id: "v1".into(),
                    region: "us".into(),
                    content_digest: hash("v1"),
                    verified_epoch: 100,
                    available: true,
                    policy_allowed: true,
                },
            ],
            policy: ArchiveMirrorPolicy {
                now_epoch: 100,
                stale_after_epochs: 10,
                required_replica_count: 2,
                require_locality: true,
                max_repair_tasks: 8,
            },
        }
    }
    #[test]
    fn healthy_replicas_synchronize() {
        let status = evaluate_glioma_distributed_archive_mirror(&request()).unwrap();
        assert_eq!(status.disposition, MirrorDisposition::Synchronized);
        status.validate().unwrap();
    }
    #[test]
    fn unauthorized_replica_blocks_repair() {
        let mut request = request();
        request.replicas[0].policy_allowed = false;
        let status = evaluate_glioma_distributed_archive_mirror(&request).unwrap();
        assert_eq!(status.disposition, MirrorDisposition::Blocked);
        assert!(!status.blocked_order.is_empty());
    }
}
