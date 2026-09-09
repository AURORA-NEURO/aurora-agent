//! Reproducibility-aware release gating for preclinical glioma research objects.
//!
//! A replay campaign is evidence about reproducibility, not an authorization to publish. This
//! feature composes the manifest and replay result into a deterministic, reviewable release
//! verdict. It never signs a bundle, moves raw data, or promotes a biological conclusion; it
//! exposes the exact gates and remediations that an accountable institution must resolve.

use super::replay::{ReplayCampaign, ReplayCampaignDisposition};
use crate::glioma::release::ReleaseStatus;
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P11-F20";
pub const OUTPUT_SCHEMA: &str = "GliomaResearchObjectReleaseGate1@1";
pub const MAX_REVIEWS: usize = 16;
pub const MAX_UNCERTAINTY_ITEMS: usize = 256;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReleaseReviewDecision {
    Approve,
    Hold,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReleaseReviewAttestation {
    pub reviewer_id: String,
    pub role: String,
    pub decision: ReleaseReviewDecision,
    pub evidence_digest: ContentHash,
    pub independent: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReleaseGateRequest {
    pub required_coverage_milli: u16,
    pub require_exact_hash: bool,
    pub require_reproducible: bool,
    pub require_accountable_review: bool,
    pub min_independent_approvals: u8,
    pub max_uncertainty_items: usize,
    pub reviews: Vec<ReleaseReviewAttestation>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReleaseGateStatus {
    Publishable,
    Hold,
    Unresolved,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReleaseGateEvaluation {
    pub feature_id: String,
    pub output_schema: String,
    pub research_id: String,
    pub study_id: String,
    pub manifest_digest: ContentHash,
    pub replay_digest: ContentHash,
    pub required_coverage_milli: u16,
    pub observed_coverage_milli: u16,
    pub exact_match: bool,
    pub replay_disposition: ReplayCampaignDisposition,
    pub release_manifest_status: ReleaseStatus,
    pub approved_review_order: Vec<String>,
    pub held_review_order: Vec<String>,
    pub blocking_order: Vec<String>,
    pub warning_order: Vec<String>,
    pub remediation_order: Vec<String>,
    pub evidence_order: Vec<String>,
    pub status: ReleaseGateStatus,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ReleaseGateError {
    #[error("release gate request is invalid: {0}")]
    InvalidRequest(String),
    #[error("replay campaign is invalid: {0}")]
    InvalidCampaign(String),
    #[error("release gate output is invalid: {0}")]
    InvalidOutput(String),
    #[error("release gate digest failed: {0}")]
    Digest(String),
}

fn canonical(values: &[String]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn valid_identifier(value: &str) -> bool {
    !value.trim().is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"._:-@>".contains(&byte))
}

fn digest_input(output: &ReleaseGateEvaluation) -> serde_json::Value {
    serde_json::json!({
        "feature_id": output.feature_id,
        "output_schema": output.output_schema,
        "research_id": output.research_id,
        "study_id": output.study_id,
        "manifest_digest": output.manifest_digest,
        "replay_digest": output.replay_digest,
        "required_coverage_milli": output.required_coverage_milli,
        "observed_coverage_milli": output.observed_coverage_milli,
        "exact_match": output.exact_match,
        "replay_disposition": output.replay_disposition,
        "release_manifest_status": output.release_manifest_status,
        "approved_review_order": output.approved_review_order,
        "held_review_order": output.held_review_order,
        "blocking_order": output.blocking_order,
        "warning_order": output.warning_order,
        "remediation_order": output.remediation_order,
        "evidence_order": output.evidence_order,
        "status": output.status,
    })
}

fn validate_request(request: &ReleaseGateRequest) -> Result<(), ReleaseGateError> {
    if request.required_coverage_milli > 1_000
        || request.max_uncertainty_items > MAX_UNCERTAINTY_ITEMS
        || request.reviews.len() > MAX_REVIEWS
        || usize::from(request.min_independent_approvals) > MAX_REVIEWS
        || (request.require_accountable_review && request.min_independent_approvals == 0)
    {
        return Err(ReleaseGateError::InvalidRequest(
            "coverage, uncertainty, review, or approval bounds are invalid".into(),
        ));
    }
    let mut reviewers = BTreeSet::new();
    for review in &request.reviews {
        if !valid_identifier(&review.reviewer_id)
            || review.role.trim().is_empty()
            || review.evidence_digest.as_str().len() != 64
            || !reviewers.insert(review.reviewer_id.clone())
        {
            return Err(ReleaseGateError::InvalidRequest(
                "reviewer identity, role, evidence digest, and uniqueness are required".into(),
            ));
        }
    }
    Ok(())
}

impl ReleaseGateEvaluation {
    pub fn validate(&self) -> Result<(), ReleaseGateError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || !valid_identifier(&self.research_id)
            || !valid_identifier(&self.study_id)
            || self.required_coverage_milli > 1_000
            || self.observed_coverage_milli > 1_000
            || !canonical(&self.approved_review_order)
            || !canonical(&self.held_review_order)
            || !canonical(&self.blocking_order)
            || !canonical(&self.warning_order)
            || !canonical(&self.remediation_order)
            || !canonical(&self.evidence_order)
            || self
                .approved_review_order
                .iter()
                .any(|id| self.held_review_order.binary_search(id).is_ok())
        {
            return Err(ReleaseGateError::InvalidOutput(
                "identity, bounds, ordering, or review partition is invalid".into(),
            ));
        }
        for digest in [&self.manifest_digest, &self.replay_digest] {
            if digest.as_str().len() != 64 {
                return Err(ReleaseGateError::InvalidOutput(
                    "manifest and replay digests must be SHA-256 values".into(),
                ));
            }
        }
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| ReleaseGateError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(ReleaseGateError::InvalidOutput(
                "release gate digest is not content-addressed".into(),
            ));
        }
        Ok(())
    }
}

/// Evaluate whether a replay campaign is ready for an accountable publication review.
///
/// This is deliberately a pure gate: it does not sign, upload, publish, or reinterpret the
/// underlying biology. A `Publishable` result means the declared release predicates are met and
/// the object may enter an institution's signing/release workflow; it is not a clinical claim.
pub fn evaluate_glioma_release_gate(
    request: &ReleaseGateRequest,
    campaign: &ReplayCampaign,
) -> Result<ReleaseGateEvaluation, ReleaseGateError> {
    validate_request(request)?;
    campaign
        .validate()
        .map_err(|error| ReleaseGateError::InvalidCampaign(error.to_string()))?;
    if campaign.research_id != campaign.manifest.research_id
        || campaign.manifest.study_id.trim().is_empty()
    {
        return Err(ReleaseGateError::InvalidCampaign(
            "campaign and manifest identity do not reconcile".into(),
        ));
    }

    let mut blocking = BTreeSet::new();
    let mut warnings = BTreeSet::new();
    let mut remediations = BTreeSet::new();
    let mut evidence = BTreeSet::new();
    let mut approved = BTreeSet::new();
    let mut held = BTreeSet::new();

    evidence.insert(format!("manifest:{}", campaign.manifest.manifest_digest));
    evidence.insert(format!("replay:{}", campaign.digest));
    if campaign.manifest.release_status == ReleaseStatus::Blocked {
        blocking.insert("manifest-blocked".into());
        remediations.insert("resolve-manifest-limitations-and-release-inputs".into());
    }
    if campaign.coverage_milli < request.required_coverage_milli {
        blocking.insert(format!(
            "coverage-below-gate:{}<{}",
            campaign.coverage_milli, request.required_coverage_milli
        ));
        remediations.insert("replay-missing-or-unavailable-tasks".into());
    }
    if request.require_exact_hash && !campaign.exact_match {
        blocking.insert("exact-hash-mismatch".into());
        remediations.insert("investigate-replay-hash-divergence".into());
    }
    if request.require_reproducible
        && campaign.disposition != ReplayCampaignDisposition::Reproducible
    {
        blocking.insert(format!("replay-disposition:{:?}", campaign.disposition));
        remediations.insert("obtain-independent-reproducible-replay".into());
    }
    if !campaign.mismatched_order.is_empty() {
        warnings.insert(format!(
            "mismatched-tasks:{}",
            campaign.mismatched_order.join(",")
        ));
    }
    if !campaign.unavailable_order.is_empty() {
        warnings.insert(format!(
            "unavailable-tasks:{}",
            campaign.unavailable_order.join(",")
        ));
    }
    if !campaign.negative_evidence.is_empty() {
        warnings.insert("negative-evidence-present".into());
        evidence.extend(
            campaign
                .negative_evidence
                .iter()
                .map(|item| format!("negative:{item}")),
        );
    }
    if campaign.uncertainty.len() > request.max_uncertainty_items {
        blocking.insert(format!(
            "uncertainty-items:{}>{}",
            campaign.uncertainty.len(),
            request.max_uncertainty_items
        ));
        remediations.insert("resolve-or-disclose-replay-uncertainty".into());
    } else {
        evidence.extend(
            campaign
                .uncertainty
                .iter()
                .map(|item| format!("uncertainty:{item}")),
        );
    }
    for review in &request.reviews {
        evidence.insert(format!("review:{}", review.evidence_digest));
        match review.decision {
            ReleaseReviewDecision::Approve if review.independent => {
                approved.insert(review.reviewer_id.clone());
            }
            ReleaseReviewDecision::Approve => {
                warnings.insert(format!("non-independent-approval:{}", review.reviewer_id));
            }
            ReleaseReviewDecision::Hold => {
                held.insert(review.reviewer_id.clone());
                blocking.insert(format!("review-hold:{}", review.reviewer_id));
                remediations.insert(format!("resolve-review-hold:{}", review.reviewer_id));
            }
        }
    }
    if request.require_accountable_review
        && approved.len() < usize::from(request.min_independent_approvals)
    {
        blocking.insert(format!(
            "independent-approvals:{}<{}",
            approved.len(),
            request.min_independent_approvals
        ));
        remediations.insert("obtain-independent-accountable-review".into());
    }
    if !request.require_accountable_review && approved.is_empty() {
        warnings.insert("no-independent-accountable-review-requested".into());
    }

    let status = if !blocking.is_empty() {
        ReleaseGateStatus::Blocked
    } else if campaign.disposition == ReplayCampaignDisposition::Unresolved
        || campaign.disposition == ReplayCampaignDisposition::Partial
    {
        ReleaseGateStatus::Unresolved
    } else if !warnings.is_empty() {
        ReleaseGateStatus::Hold
    } else {
        ReleaseGateStatus::Publishable
    };
    if status != ReleaseGateStatus::Publishable {
        remediations.insert("complete-accountable-release-review-before-publication".into());
    }
    let mut output = ReleaseGateEvaluation {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        research_id: campaign.research_id.clone(),
        study_id: campaign.manifest.study_id.clone(),
        manifest_digest: campaign.manifest.manifest_digest.clone(),
        replay_digest: campaign.digest.clone(),
        required_coverage_milli: request.required_coverage_milli,
        observed_coverage_milli: campaign.coverage_milli,
        exact_match: campaign.exact_match,
        replay_disposition: campaign.disposition,
        release_manifest_status: campaign.manifest.release_status,
        approved_review_order: approved.into_iter().collect(),
        held_review_order: held.into_iter().collect(),
        blocking_order: blocking.into_iter().collect(),
        warning_order: warnings.into_iter().collect(),
        remediation_order: remediations.into_iter().collect(),
        evidence_order: evidence.into_iter().collect(),
        status,
        digest: ContentHash::of_bytes(b"unsealed-glioma-release-gate"),
    };
    output.digest = ContentHash::of_value(&digest_input(&output))
        .map_err(|error| ReleaseGateError::Digest(error.to_string()))?;
    output.validate()?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::glioma::programs::p11_research_object_release::replay::{
        execute_glioma_replay_campaign, DryRunReplayCampaignExecutor, ReplayCampaignRequest,
        ReplayTask,
    };
    use crate::glioma::release::ResearchObjectRequest;
    use crate::glioma_engine::LocalArtifactRef;

    fn hash(label: &str) -> ContentHash {
        ContentHash::of_value(&serde_json::json!({"label": label})).unwrap()
    }

    fn request() -> ReplayCampaignRequest {
        ReplayCampaignRequest {
            release: ResearchObjectRequest {
                research_id: "gate-research".into(),
                study_id: "gate-study".into(),
                objective: "release a preclinical glioma result".into(),
                plan_digest: hash("plan"),
                execution_digest: hash("execution"),
                replay_identity: hash("replay"),
                program_order: vec!["p05-mechanism".into(), "p10-analysis".into()],
                artifacts: vec![LocalArtifactRef {
                    artifact_id: "artifact-main".into(),
                    content_hash: hash("artifact-main"),
                    content_type: "application/json".into(),
                    local_only: true,
                    contains_human_data: false,
                    contains_direct_identifiers: false,
                }],
                negative_evidence: vec!["null-result-preserved".into()],
                limitations: vec!["single-model-system".into()],
                raw_data_local: true,
                aggregate_only: true,
            },
            tasks: vec![
                ReplayTask {
                    task_id: "mechanism-replay".into(),
                    program_id: "p05-mechanism".into(),
                    artifact_id: "artifact-main".into(),
                    expected_content_hash: hash("artifact-main"),
                    cost_units: 1,
                    required: true,
                    deterministic: true,
                    depends_on: Vec::new(),
                },
                ReplayTask {
                    task_id: "analysis-replay".into(),
                    program_id: "p10-analysis".into(),
                    artifact_id: "artifact-analysis".into(),
                    expected_content_hash: hash("artifact-analysis"),
                    cost_units: 1,
                    required: true,
                    deterministic: true,
                    depends_on: vec!["mechanism-replay".into()],
                },
            ],
            budget_units: 2,
            max_rounds: 3,
            max_retries: 1,
            min_coverage_milli: 1_000,
            require_exact_hash: true,
        }
    }

    fn gate_request() -> ReleaseGateRequest {
        ReleaseGateRequest {
            required_coverage_milli: 1_000,
            require_exact_hash: true,
            require_reproducible: true,
            require_accountable_review: true,
            min_independent_approvals: 1,
            max_uncertainty_items: 8,
            reviews: vec![ReleaseReviewAttestation {
                reviewer_id: "reviewer-a".into(),
                role: "independent-reproducibility-reviewer".into(),
                decision: ReleaseReviewDecision::Approve,
                evidence_digest: hash("review-a"),
                independent: true,
            }],
        }
    }

    fn campaign() -> ReplayCampaign {
        let mut executor = DryRunReplayCampaignExecutor;
        execute_glioma_replay_campaign(&request(), &mut executor).unwrap()
    }

    #[test]
    fn publishable_gate_requires_replay_and_accountable_review() {
        let output = evaluate_glioma_release_gate(&gate_request(), &campaign()).unwrap();
        assert_eq!(output.status, ReleaseGateStatus::Publishable);
        assert_eq!(output.observed_coverage_milli, 1_000);
        assert!(output.blocking_order.is_empty());
        output.validate().unwrap();
    }

    #[test]
    fn held_review_blocks_even_a_reproducible_campaign() {
        let mut request = gate_request();
        request.reviews[0].decision = ReleaseReviewDecision::Hold;
        let output = evaluate_glioma_release_gate(&request, &campaign()).unwrap();
        assert_eq!(output.status, ReleaseGateStatus::Blocked);
        assert!(output
            .blocking_order
            .iter()
            .any(|item| item == "review-hold:reviewer-a"));
    }

    #[test]
    fn missing_approval_is_explicit_and_replay_stays_unpromoted() {
        let mut request = gate_request();
        request.reviews.clear();
        let output = evaluate_glioma_release_gate(&request, &campaign()).unwrap();
        assert_eq!(output.status, ReleaseGateStatus::Blocked);
        assert!(output
            .blocking_order
            .iter()
            .any(|item| item.starts_with("independent-approvals:")));
    }

    #[test]
    fn release_gate_is_replay_stable_under_review_permutation() {
        let replay = campaign();
        let mut first = gate_request();
        first.reviews.push(ReleaseReviewAttestation {
            reviewer_id: "reviewer-b".into(),
            role: "methods-reviewer".into(),
            decision: ReleaseReviewDecision::Approve,
            evidence_digest: hash("review-b"),
            independent: true,
        });
        let mut second = first.clone();
        second.reviews.reverse();
        assert_eq!(
            evaluate_glioma_release_gate(&first, &replay).unwrap(),
            evaluate_glioma_release_gate(&second, &replay).unwrap()
        );
    }
}
