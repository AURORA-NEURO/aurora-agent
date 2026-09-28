//! Immutable-version retention planning for preclinical glioma research objects.
//!
//! This feature turns preservation policy into an auditable, deterministic action plan. It never
//! rewrites a signed version or deletes bytes; legal holds, pins, lineage, replica health, and
//! digest verification are evaluated independently so an administrator can approve safe archive
//! transitions while unsafe deletion and restoration remain explicit blocks.

use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P11-F29";
pub const OUTPUT_SCHEMA: &str = "GliomaReleaseRetentionPlan1@1";
pub const MAX_VERSIONS: usize = 4_096;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReleaseVersion {
    pub version_id: String,
    pub object_id: String,
    pub content_digest: ContentHash,
    pub predecessor_version_id: Option<String>,
    pub issued_epoch: u64,
    pub immutable: bool,
    pub pinned: bool,
    pub legal_hold: bool,
    pub superseded_by: Option<String>,
    pub storage_tier: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VersionStorageHealth {
    pub version_id: String,
    pub verified_replica_count: u16,
    pub last_verified_epoch: u64,
    pub digest_matches_source: bool,
    pub approved_region_order: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RetentionPolicy {
    pub now_epoch: u64,
    pub archive_after_epochs: u64,
    pub minimum_retain_epochs: u64,
    pub minimum_verified_replicas: u16,
    pub allow_delete_plan: bool,
    pub require_immutable_versions: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RetentionGovernorRequest {
    pub versions: Vec<ReleaseVersion>,
    pub storage_health: Vec<VersionStorageHealth>,
    pub policy: RetentionPolicy,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RetentionAction {
    Retain,
    Archive,
    DeleteBlocked,
    RestoreVerificationBlocked,
    Unresolved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RetentionDecision {
    pub version_id: String,
    pub action: RetentionAction,
    pub age_epochs: u64,
    pub rationale_order: Vec<String>,
    pub verified_replica_count: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RetentionDisposition {
    Planned,
    Partial,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RetentionActionPlan {
    pub feature_id: String,
    pub output_schema: String,
    pub decisions: Vec<RetentionDecision>,
    pub protected_version_order: Vec<String>,
    pub archive_version_order: Vec<String>,
    pub delete_blocked_order: Vec<String>,
    pub unresolved_order: Vec<String>,
    pub disposition: RetentionDisposition,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum RetentionGovernorError {
    #[error("retention request is invalid: {0}")]
    InvalidRequest(String),
    #[error("retention output is invalid: {0}")]
    InvalidOutput(String),
    #[error("retention digest failed: {0}")]
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

fn digest_input(plan: &RetentionActionPlan) -> serde_json::Value {
    serde_json::json!({
        "feature_id": plan.feature_id,
        "output_schema": plan.output_schema,
        "decisions": plan.decisions,
        "protected_version_order": plan.protected_version_order,
        "archive_version_order": plan.archive_version_order,
        "delete_blocked_order": plan.delete_blocked_order,
        "unresolved_order": plan.unresolved_order,
        "disposition": plan.disposition,
    })
}

fn validate_request(request: &RetentionGovernorRequest) -> Result<(), RetentionGovernorError> {
    if request.versions.is_empty()
        || request.versions.len() > MAX_VERSIONS
        || request.storage_health.len() > MAX_VERSIONS
        || request.policy.now_epoch == 0
        || request.policy.minimum_verified_replicas == 0
        || request.versions.iter().any(|version| {
            !valid_id(&version.version_id)
                || !valid_id(&version.object_id)
                || !valid_digest(&version.content_digest)
                || version.issued_epoch == 0
                || version.issued_epoch > request.policy.now_epoch
                || version
                    .predecessor_version_id
                    .as_ref()
                    .is_some_and(|value| !valid_id(value))
                || version
                    .superseded_by
                    .as_ref()
                    .is_some_and(|value| !valid_id(value))
                || !valid_id(&version.storage_tier)
        })
        || request.storage_health.iter().any(|health| {
            !valid_id(&health.version_id)
                || health.last_verified_epoch > request.policy.now_epoch
                || health
                    .approved_region_order
                    .windows(2)
                    .any(|pair| pair[0] >= pair[1])
                || health
                    .approved_region_order
                    .iter()
                    .any(|region| !valid_id(region))
        })
    {
        return Err(RetentionGovernorError::InvalidRequest(
            "bounded versions, epochs, digests, storage tiers, and canonical regions are required"
                .into(),
        ));
    }
    let mut ids = BTreeSet::new();
    if request
        .versions
        .iter()
        .any(|version| !ids.insert(version.version_id.clone()))
    {
        return Err(RetentionGovernorError::InvalidRequest(
            "version identifiers must be unique".into(),
        ));
    }
    Ok(())
}

impl RetentionActionPlan {
    pub fn validate(&self) -> Result<(), RetentionGovernorError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || self
                .decisions
                .windows(2)
                .any(|pair| pair[0].version_id >= pair[1].version_id)
            || self.decisions.iter().any(|decision| {
                !valid_id(&decision.version_id)
                    || decision
                        .rationale_order
                        .iter()
                        .any(|reason| reason.is_empty())
            })
        {
            return Err(RetentionGovernorError::InvalidOutput(
                "retention identity, decision ordering, or rationale is invalid".into(),
            ));
        }
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| RetentionGovernorError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(RetentionGovernorError::InvalidOutput(
                "retention plan digest is not content-addressed".into(),
            ));
        }
        Ok(())
    }
}

/// Produce a deterministic retention/archive plan without mutating or deleting any version.
pub fn plan_glioma_version_retention(
    request: &RetentionGovernorRequest,
) -> Result<RetentionActionPlan, RetentionGovernorError> {
    validate_request(request)?;
    let mut health_by_id = request
        .storage_health
        .iter()
        .map(|health| (health.version_id.clone(), health))
        .collect::<std::collections::BTreeMap<_, _>>();
    let mut decisions = Vec::new();
    for version in &request.versions {
        let age = request.policy.now_epoch - version.issued_epoch;
        let mut rationale = BTreeSet::new();
        let Some(health) = health_by_id.remove(&version.version_id) else {
            decisions.push(RetentionDecision {
                version_id: version.version_id.clone(),
                action: RetentionAction::Unresolved,
                age_epochs: age,
                rationale_order: vec!["missing-storage-health".into()],
                verified_replica_count: 0,
            });
            continue;
        };
        if !health.digest_matches_source || health.verified_replica_count == 0 {
            decisions.push(RetentionDecision {
                version_id: version.version_id.clone(),
                action: RetentionAction::RestoreVerificationBlocked,
                age_epochs: age,
                rationale_order: vec!["digest-or-replica-verification-failed".into()],
                verified_replica_count: health.verified_replica_count,
            });
            continue;
        }
        if version.pinned || version.legal_hold {
            rationale.insert(
                if version.pinned {
                    "pinned"
                } else {
                    "legal-hold"
                }
                .into(),
            );
            decisions.push(RetentionDecision {
                version_id: version.version_id.clone(),
                action: RetentionAction::Retain,
                age_epochs: age,
                rationale_order: rationale.into_iter().collect(),
                verified_replica_count: health.verified_replica_count,
            });
            continue;
        }
        if request.policy.require_immutable_versions && !version.immutable {
            decisions.push(RetentionDecision {
                version_id: version.version_id.clone(),
                action: RetentionAction::DeleteBlocked,
                age_epochs: age,
                rationale_order: vec!["version-not-immutable".into()],
                verified_replica_count: health.verified_replica_count,
            });
            continue;
        }
        if age >= request.policy.archive_after_epochs
            && health.verified_replica_count >= request.policy.minimum_verified_replicas
        {
            decisions.push(RetentionDecision {
                version_id: version.version_id.clone(),
                action: RetentionAction::Archive,
                age_epochs: age,
                rationale_order: vec!["archive-age-and-replica-gate-passed".into()],
                verified_replica_count: health.verified_replica_count,
            });
        } else if request.policy.allow_delete_plan
            && age >= request.policy.minimum_retain_epochs
            && version.superseded_by.is_some()
            && health.verified_replica_count >= request.policy.minimum_verified_replicas
        {
            decisions.push(RetentionDecision {
                version_id: version.version_id.clone(),
                action: RetentionAction::DeleteBlocked,
                age_epochs: age,
                rationale_order: vec!["deletion-requires-explicit-administrator-approval".into()],
                verified_replica_count: health.verified_replica_count,
            });
        } else {
            decisions.push(RetentionDecision {
                version_id: version.version_id.clone(),
                action: RetentionAction::Retain,
                age_epochs: age,
                rationale_order: vec!["retention-window-active".into()],
                verified_replica_count: health.verified_replica_count,
            });
        }
    }
    decisions.sort_by(|left, right| left.version_id.cmp(&right.version_id));
    let protected = decisions
        .iter()
        .filter(|decision| matches!(decision.action, RetentionAction::Retain))
        .map(|decision| decision.version_id.clone())
        .collect::<Vec<_>>();
    let archive = decisions
        .iter()
        .filter(|decision| matches!(decision.action, RetentionAction::Archive))
        .map(|decision| decision.version_id.clone())
        .collect::<Vec<_>>();
    let delete_blocked = decisions
        .iter()
        .filter(|decision| matches!(decision.action, RetentionAction::DeleteBlocked))
        .map(|decision| decision.version_id.clone())
        .collect::<Vec<_>>();
    let unresolved = decisions
        .iter()
        .filter(|decision| {
            matches!(
                decision.action,
                RetentionAction::Unresolved | RetentionAction::RestoreVerificationBlocked
            )
        })
        .map(|decision| decision.version_id.clone())
        .collect::<Vec<_>>();
    let disposition = if !unresolved.is_empty() {
        RetentionDisposition::Blocked
    } else if !delete_blocked.is_empty() {
        RetentionDisposition::Partial
    } else {
        RetentionDisposition::Planned
    };
    let mut plan = RetentionActionPlan {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        decisions,
        protected_version_order: protected,
        archive_version_order: archive,
        delete_blocked_order: delete_blocked,
        unresolved_order: unresolved,
        disposition,
        digest: ContentHash::of_bytes(b"unsealed-glioma-retention-plan"),
    };
    plan.digest = ContentHash::of_value(&digest_input(&plan))
        .map_err(|error| RetentionGovernorError::Digest(error.to_string()))?;
    plan.validate()?;
    Ok(plan)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hash(label: &str) -> ContentHash {
        ContentHash::of_value(&serde_json::json!({"label": label})).unwrap()
    }

    fn request() -> RetentionGovernorRequest {
        RetentionGovernorRequest {
            versions: vec![
                ReleaseVersion {
                    version_id: "v1".into(),
                    object_id: "object".into(),
                    content_digest: hash("v1"),
                    predecessor_version_id: None,
                    issued_epoch: 1,
                    immutable: true,
                    pinned: true,
                    legal_hold: false,
                    superseded_by: Some("v2".into()),
                    storage_tier: "hot".into(),
                },
                ReleaseVersion {
                    version_id: "v2".into(),
                    object_id: "object".into(),
                    content_digest: hash("v2"),
                    predecessor_version_id: Some("v1".into()),
                    issued_epoch: 50,
                    immutable: true,
                    pinned: false,
                    legal_hold: false,
                    superseded_by: None,
                    storage_tier: "hot".into(),
                },
            ],
            storage_health: vec![
                VersionStorageHealth {
                    version_id: "v1".into(),
                    verified_replica_count: 2,
                    last_verified_epoch: 100,
                    digest_matches_source: true,
                    approved_region_order: vec!["eu".into(), "us".into()],
                },
                VersionStorageHealth {
                    version_id: "v2".into(),
                    verified_replica_count: 2,
                    last_verified_epoch: 100,
                    digest_matches_source: true,
                    approved_region_order: vec!["eu".into()],
                },
            ],
            policy: RetentionPolicy {
                now_epoch: 100,
                archive_after_epochs: 40,
                minimum_retain_epochs: 10,
                minimum_verified_replicas: 2,
                allow_delete_plan: true,
                require_immutable_versions: true,
            },
        }
    }

    #[test]
    fn legal_hold_and_pinned_versions_are_retained() {
        let plan = plan_glioma_version_retention(&request()).unwrap();
        assert_eq!(plan.disposition, RetentionDisposition::Planned);
        assert_eq!(plan.protected_version_order, vec!["v1"]);
        assert_eq!(plan.archive_version_order, vec!["v2"]);
        plan.validate().unwrap();
    }

    #[test]
    fn missing_or_tampered_health_blocks_plan() {
        let mut request = request();
        request.storage_health[1].digest_matches_source = false;
        let plan = plan_glioma_version_retention(&request).unwrap();
        assert_eq!(plan.disposition, RetentionDisposition::Blocked);
        assert!(plan.unresolved_order.contains(&"v2".into()));
    }
}
