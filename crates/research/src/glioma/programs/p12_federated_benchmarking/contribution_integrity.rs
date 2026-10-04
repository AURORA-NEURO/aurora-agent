//! Contribution-integrity verification for aggregate-only federated glioma research.
//!
//! This feature is the reusable trust gate before quorum admission or any statistic. It checks
//! declared signer identity, signature validity, policy scope, schema and benchmark binding,
//! freshness, revocation, locality, and duplicate artifact identity. It returns stable rejection
//! reasons instead of silently dropping inputs. Verification is over typed declarations and trust
//! snapshots; it never moves raw data, performs a clinical decision, or treats a declaration as a
//! biological result.

use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P12-F25";
pub const OUTPUT_SCHEMA: &str = "GliomaFederatedContributionIntegrity1@1";
pub const MAX_CONTRIBUTIONS: usize = 512;
pub const MAX_REASON_LENGTH: usize = 256;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContributionIntegrityStatus {
    Verified,
    Rejected,
    Unresolved,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContributionIntegrityReason {
    DuplicateContributionId,
    DuplicateSiteArtifact,
    MissingSigner,
    InvalidSignature,
    RevokedSite,
    ApprovalMissing,
    PolicyScopeMismatch,
    SchemaMismatch,
    BenchmarkMismatch,
    StaleContribution,
    FutureContribution,
    HumanDataDeclared,
    DirectIdentifiersDeclared,
    RawDataNotLocal,
    NonAggregateContribution,
    MissingArtifactDigest,
    MissingAggregateDigest,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContributionIntegrityRecord {
    pub contribution_id: String,
    pub site_id: String,
    pub study_id: String,
    pub benchmark_id: String,
    pub policy_scope: String,
    pub schema_version: String,
    pub signer_id: Option<String>,
    pub signature_digest: Option<ContentHash>,
    pub signature_valid: bool,
    pub approved: bool,
    pub revoked: bool,
    pub observed_tick: u64,
    pub artifact_digest: Option<ContentHash>,
    pub aggregate_digest: Option<ContentHash>,
    pub aggregate_only: bool,
    pub raw_data_local: bool,
    pub contains_human_data: bool,
    pub contains_direct_identifiers: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContributionIntegrityRequest {
    pub benchmark_id: String,
    pub required_policy_scope: String,
    pub required_schema_version: String,
    pub current_tick: u64,
    pub max_staleness_ticks: u64,
    pub require_signatures: bool,
    pub contributions: Vec<ContributionIntegrityRecord>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContributionIntegrityReview {
    pub contribution_id: String,
    pub site_id: String,
    pub study_id: String,
    pub status: ContributionIntegrityStatus,
    pub reason: Option<ContributionIntegrityReason>,
    pub artifact_digest: Option<ContentHash>,
    pub age_ticks: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederatedContributionIntegrity {
    pub feature_id: String,
    pub output_schema: String,
    pub benchmark_id: String,
    pub verified_contribution_order: Vec<String>,
    pub rejected_contribution_order: Vec<String>,
    pub unresolved_contribution_order: Vec<String>,
    pub duplicate_artifact_order: Vec<String>,
    pub reviews: Vec<ContributionIntegrityReview>,
    pub trust_snapshot_digest: ContentHash,
    pub verified_count: usize,
    pub rejected_count: usize,
    pub unresolved_count: usize,
    pub blockers: Vec<String>,
    pub status: ContributionIntegrityStatus,
    pub aggregate_consumption_permitted: bool,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ContributionIntegrityError {
    #[error("contribution-integrity request is invalid: {0}")]
    InvalidRequest(String),
    #[error("contribution-integrity output is invalid: {0}")]
    InvalidOutput(String),
    #[error("contribution-integrity digest failed: {0}")]
    Digest(String),
}

fn safe_text(value: &str) -> bool {
    !value.trim().is_empty()
        && value.len() <= MAX_REASON_LENGTH
        && !value.chars().any(char::is_control)
}

fn canonical(values: &[String]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn digest_body(output: &FederatedContributionIntegrity) -> serde_json::Value {
    serde_json::json!({
        "feature_id": output.feature_id,
        "output_schema": output.output_schema,
        "benchmark_id": output.benchmark_id,
        "verified_contribution_order": output.verified_contribution_order,
        "rejected_contribution_order": output.rejected_contribution_order,
        "unresolved_contribution_order": output.unresolved_contribution_order,
        "duplicate_artifact_order": output.duplicate_artifact_order,
        "reviews": output.reviews,
        "trust_snapshot_digest": output.trust_snapshot_digest,
        "verified_count": output.verified_count,
        "rejected_count": output.rejected_count,
        "unresolved_count": output.unresolved_count,
        "blockers": output.blockers,
        "status": output.status,
        "aggregate_consumption_permitted": output.aggregate_consumption_permitted,
    })
}

fn validate_request(
    request: &ContributionIntegrityRequest,
) -> Result<(), ContributionIntegrityError> {
    if !safe_text(&request.benchmark_id)
        || !safe_text(&request.required_policy_scope)
        || !safe_text(&request.required_schema_version)
        || request.max_staleness_ticks == 0
        || request.contributions.is_empty()
        || request.contributions.len() > MAX_CONTRIBUTIONS
    {
        return Err(ContributionIntegrityError::InvalidRequest(
            "bounded benchmark identity, policy/schema, freshness, and contribution inputs are required".into(),
        ));
    }
    for contribution in &request.contributions {
        if !safe_text(&contribution.contribution_id)
            || !safe_text(&contribution.site_id)
            || !safe_text(&contribution.study_id)
            || !safe_text(&contribution.benchmark_id)
        {
            return Err(ContributionIntegrityError::InvalidRequest(
                "contribution identity fields must be bounded and non-empty".into(),
            ));
        }
    }
    Ok(())
}

impl FederatedContributionIntegrity {
    pub fn validate(&self) -> Result<(), ContributionIntegrityError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || !safe_text(&self.benchmark_id)
            || !canonical(&self.verified_contribution_order)
            || !canonical(&self.rejected_contribution_order)
            || !canonical(&self.unresolved_contribution_order)
            || !canonical(&self.duplicate_artifact_order)
            || !canonical(&self.blockers)
            || self.verified_count != self.verified_contribution_order.len()
            || self.rejected_count != self.rejected_contribution_order.len()
            || self.unresolved_count != self.unresolved_contribution_order.len()
            || self.trust_snapshot_digest.as_str().len() != 64
            || self.digest.as_str().len() != 64
            || self.reviews.iter().any(|review| {
                !safe_text(&review.contribution_id)
                    || !safe_text(&review.site_id)
                    || !safe_text(&review.study_id)
                    || review.status == ContributionIntegrityStatus::Verified
                        && review.reason.is_some()
                    || review.status != ContributionIntegrityStatus::Verified
                        && review.reason.is_none()
            })
        {
            return Err(ContributionIntegrityError::InvalidOutput(
                "integrity identity, ordering, counts, review, trust snapshot, or digest invariants are invalid".into(),
            ));
        }
        let expected = ContentHash::of_value(&digest_body(self))
            .map_err(|error| ContributionIntegrityError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(ContributionIntegrityError::InvalidOutput(
                "contribution-integrity digest is not content-bound".into(),
            ));
        }
        Ok(())
    }
}

/// Verify typed contribution declarations before a federated statistic can consume them.
pub fn verify_glioma_contribution_integrity(
    request: &ContributionIntegrityRequest,
) -> Result<FederatedContributionIntegrity, ContributionIntegrityError> {
    validate_request(request)?;
    let mut contributions = request.contributions.clone();
    contributions.sort_by(|left, right| left.contribution_id.cmp(&right.contribution_id));
    let mut seen_ids = BTreeSet::new();
    let mut seen_artifacts = BTreeSet::new();
    let mut verified = BTreeSet::new();
    let mut rejected = BTreeSet::new();
    let unresolved = BTreeSet::new();
    let mut duplicate_artifacts = BTreeSet::new();
    let mut reviews = Vec::with_capacity(contributions.len());

    for contribution in &contributions {
        let mut reason = None;
        let age_ticks = if contribution.observed_tick > request.current_tick {
            reason = Some(ContributionIntegrityReason::FutureContribution);
            None
        } else {
            let age = request.current_tick - contribution.observed_tick;
            if age > request.max_staleness_ticks {
                reason = Some(ContributionIntegrityReason::StaleContribution);
            }
            Some(age)
        };
        if !seen_ids.insert(contribution.contribution_id.clone()) {
            reason = Some(ContributionIntegrityReason::DuplicateContributionId);
        } else if contribution.benchmark_id != request.benchmark_id {
            reason = Some(ContributionIntegrityReason::BenchmarkMismatch);
        } else if !contribution.approved {
            reason = Some(ContributionIntegrityReason::ApprovalMissing);
        } else if contribution.revoked {
            reason = Some(ContributionIntegrityReason::RevokedSite);
        } else if request.require_signatures
            && contribution
                .signer_id
                .as_deref()
                .is_none_or(|signer| !safe_text(signer))
        {
            reason = Some(ContributionIntegrityReason::MissingSigner);
        } else if request.require_signatures
            && (contribution.signature_digest.is_none() || !contribution.signature_valid)
        {
            reason = Some(ContributionIntegrityReason::InvalidSignature);
        } else if contribution.policy_scope != request.required_policy_scope {
            reason = Some(ContributionIntegrityReason::PolicyScopeMismatch);
        } else if contribution.schema_version != request.required_schema_version {
            reason = Some(ContributionIntegrityReason::SchemaMismatch);
        } else if contribution.contains_human_data {
            reason = Some(ContributionIntegrityReason::HumanDataDeclared);
        } else if contribution.contains_direct_identifiers {
            reason = Some(ContributionIntegrityReason::DirectIdentifiersDeclared);
        } else if !contribution.raw_data_local {
            reason = Some(ContributionIntegrityReason::RawDataNotLocal);
        } else if !contribution.aggregate_only {
            reason = Some(ContributionIntegrityReason::NonAggregateContribution);
        } else if contribution.artifact_digest.is_none() {
            reason = Some(ContributionIntegrityReason::MissingArtifactDigest);
        } else if contribution.aggregate_digest.is_none() {
            reason = Some(ContributionIntegrityReason::MissingAggregateDigest);
        } else if !seen_artifacts.insert((
            contribution.site_id.clone(),
            contribution.study_id.clone(),
            contribution.artifact_digest.clone(),
        )) {
            duplicate_artifacts.insert(contribution.contribution_id.clone());
            reason = Some(ContributionIntegrityReason::DuplicateSiteArtifact);
        }

        let status = if reason.is_some() {
            rejected.insert(contribution.contribution_id.clone());
            ContributionIntegrityStatus::Rejected
        } else {
            verified.insert(contribution.contribution_id.clone());
            ContributionIntegrityStatus::Verified
        };
        reviews.push(ContributionIntegrityReview {
            contribution_id: contribution.contribution_id.clone(),
            site_id: contribution.site_id.clone(),
            study_id: contribution.study_id.clone(),
            status,
            reason,
            artifact_digest: contribution.artifact_digest.clone(),
            age_ticks,
        });
    }

    let verified_order = verified.into_iter().collect::<Vec<_>>();
    let rejected_order = rejected.into_iter().collect::<Vec<_>>();
    let unresolved_order = unresolved.into_iter().collect::<Vec<_>>();
    let duplicate_artifact_order = duplicate_artifacts.into_iter().collect::<Vec<_>>();
    let mut blockers = Vec::new();
    if verified_order.is_empty() {
        blockers.push("no contribution passed integrity verification".into());
    }
    if !rejected_order.is_empty() {
        blockers.push(format!(
            "{} contribution(s) were rejected with stable reasons",
            rejected_order.len()
        ));
    }
    blockers.sort();
    let status = if verified_order.is_empty() {
        ContributionIntegrityStatus::Rejected
    } else if !rejected_order.is_empty() {
        ContributionIntegrityStatus::Unresolved
    } else {
        ContributionIntegrityStatus::Verified
    };
    let mut output = FederatedContributionIntegrity {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        benchmark_id: request.benchmark_id.clone(),
        verified_contribution_order: verified_order,
        rejected_contribution_order: rejected_order,
        unresolved_contribution_order: unresolved_order,
        duplicate_artifact_order,
        reviews,
        trust_snapshot_digest: ContentHash::of_value(&serde_json::json!({
            "benchmark_id": request.benchmark_id.clone(),
            "required_policy_scope": request.required_policy_scope.clone(),
            "required_schema_version": request.required_schema_version.clone(),
            "current_tick": request.current_tick,
            "max_staleness_ticks": request.max_staleness_ticks,
            "require_signatures": request.require_signatures,
        }))
        .map_err(|error| ContributionIntegrityError::Digest(error.to_string()))?,
        verified_count: 0,
        rejected_count: 0,
        unresolved_count: 0,
        blockers,
        status,
        aggregate_consumption_permitted: status == ContributionIntegrityStatus::Verified,
        digest: ContentHash::of_bytes(b"unsealed-glioma-contribution-integrity"),
    };
    output.verified_count = output.verified_contribution_order.len();
    output.rejected_count = output.rejected_contribution_order.len();
    output.unresolved_count = output.unresolved_contribution_order.len();
    output.digest = ContentHash::of_value(&digest_body(&output))
        .map_err(|error| ContributionIntegrityError::Digest(error.to_string()))?;
    output.validate()?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn digest(label: &str) -> ContentHash {
        ContentHash::of_value(&serde_json::json!({"label": label})).unwrap()
    }

    fn record(id: &str, site: &str) -> ContributionIntegrityRecord {
        ContributionIntegrityRecord {
            contribution_id: id.into(),
            site_id: site.into(),
            study_id: format!("study-{site}"),
            benchmark_id: "benchmark-1".into(),
            policy_scope: "glioma-v1".into(),
            schema_version: "aggregate-v1".into(),
            signer_id: Some(format!("signer-{site}")),
            signature_digest: Some(digest(&format!("signature-{id}"))),
            signature_valid: true,
            approved: true,
            revoked: false,
            observed_tick: 95,
            artifact_digest: Some(digest(&format!("artifact-{id}"))),
            aggregate_digest: Some(digest(&format!("aggregate-{id}"))),
            aggregate_only: true,
            raw_data_local: true,
            contains_human_data: false,
            contains_direct_identifiers: false,
        }
    }

    fn request(contributions: Vec<ContributionIntegrityRecord>) -> ContributionIntegrityRequest {
        ContributionIntegrityRequest {
            benchmark_id: "benchmark-1".into(),
            required_policy_scope: "glioma-v1".into(),
            required_schema_version: "aggregate-v1".into(),
            current_tick: 100,
            max_staleness_ticks: 10,
            require_signatures: true,
            contributions,
        }
    }

    #[test]
    fn valid_contributions_are_verified() {
        let output =
            verify_glioma_contribution_integrity(&request(vec![record("a", "site-a")])).unwrap();
        assert_eq!(output.status, ContributionIntegrityStatus::Verified);
        assert!(output.aggregate_consumption_permitted);
        assert_eq!(output.verified_contribution_order, vec!["a"]);
    }

    #[test]
    fn stale_revoked_and_invalid_schema_contributions_are_rejected() {
        let mut stale = record("a", "site-a");
        stale.observed_tick = 50;
        let mut revoked = record("b", "site-b");
        revoked.revoked = true;
        let mut schema = record("c", "site-c");
        schema.schema_version = "old".into();
        let output =
            verify_glioma_contribution_integrity(&request(vec![stale, revoked, schema])).unwrap();
        assert_eq!(output.status, ContributionIntegrityStatus::Rejected);
        assert_eq!(output.rejected_count, 3);
        assert!(output.reviews.iter().any(|review| {
            review.reason == Some(ContributionIntegrityReason::StaleContribution)
        }));
    }

    #[test]
    fn duplicate_artifact_and_signature_failures_are_explicit() {
        let first = record("a", "site-a");
        let mut duplicate = record("b", "site-b");
        duplicate.site_id = first.site_id.clone();
        duplicate.artifact_digest = first.artifact_digest.clone();
        duplicate.study_id = first.study_id.clone();
        let mut unsigned = record("c", "site-c");
        unsigned.signature_digest = None;
        unsigned.signature_valid = false;
        let output =
            verify_glioma_contribution_integrity(&request(vec![first, duplicate, unsigned]))
                .unwrap();
        assert_eq!(output.status, ContributionIntegrityStatus::Unresolved);
        assert_eq!(output.duplicate_artifact_order, vec!["b"]);
        assert!(output.reviews.iter().any(|review| {
            review.reason == Some(ContributionIntegrityReason::InvalidSignature)
        }));
    }

    #[test]
    fn protected_or_nonaggregate_inputs_cannot_be_consumed() {
        let mut human = record("a", "site-a");
        human.contains_human_data = true;
        let mut raw = record("b", "site-b");
        raw.raw_data_local = false;
        let mut nonaggregate = record("c", "site-c");
        nonaggregate.aggregate_only = false;
        let output =
            verify_glioma_contribution_integrity(&request(vec![human, raw, nonaggregate])).unwrap();
        assert!(!output.aggregate_consumption_permitted);
        assert_eq!(output.rejected_count, 3);
    }

    #[test]
    fn identical_replay_is_deterministic() {
        let req = request(vec![record("b", "site-b"), record("a", "site-a")]);
        let first = verify_glioma_contribution_integrity(&req).unwrap();
        let second = verify_glioma_contribution_integrity(&req).unwrap();
        assert_eq!(first, second);
        assert_eq!(first.digest, second.digest);
    }
}
