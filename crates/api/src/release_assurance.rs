//! Prospective high-throughput research-release assurance.
//!
//! Atlas feature: `AFA-api-P16-F27`.
//!
//! The API surface verifies already-produced release candidates before a publication service may
//! accept them. It is deliberately not a signer or transport: it returns a typed local receipt,
//! retains unresolved and negative evidence, and fails closed on protected-closure, replay,
//! provenance, policy, approval, locality, benchmark, or budget gaps.

use bioprism_foundation::{
    AuthorityRequirement, AutonomyTier, CapabilityManifest, Determinism, Effect, EvidenceReference,
    EvidenceState, ResearchSurface, TypedPort, TypedResearchArtifact, PRECLINICAL_BOUNDARY,
    RESEARCH_CONTRACT_SCHEMA_VERSION,
};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-api-P16-F27";
pub const CONTRACT_VERSION: &str = "api-publication-release-assurance/1.0";
pub const MAX_CANDIDATES: usize = 4096;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReleaseState {
    Supported,
    Unknown,
    Contradicted,
    Unmeasured,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReleaseDisposition {
    Qualified,
    Partial,
    Unknown,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ValidatedResearchRun {
    pub run_id: String,
    pub release_id: String,
    pub scope: String,
    pub artifact_ids: Vec<String>,
    pub evidence_receipt_ids: Vec<String>,
    pub release_digest: ContentHash,
    pub provenance_digest: ContentHash,
    pub replay_identity: ContentHash,
    pub benchmark_digest: Option<ContentHash>,
    pub state: ReleaseState,
    pub omissions: Vec<String>,
    pub uncertainty: Vec<String>,
    pub negative_evidence: Vec<String>,
    pub raw_data_local: bool,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReleaseAssuranceRequest {
    pub request_id: String,
    pub workflow_id: String,
    pub scope: String,
    pub candidates: Vec<ValidatedResearchRun>,
    pub replay_identity: ContentHash,
    pub benchmark_digest: Option<ContentHash>,
    pub max_admissions: usize,
    pub budget: u64,
    pub policy_allow: bool,
    pub protected_closure: bool,
    pub signed_approval: bool,
    pub raw_data_local: bool,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SignedResearchObject {
    pub run_id: String,
    pub release_id: String,
    pub artifact_ids: Vec<String>,
    pub evidence_receipt_ids: Vec<String>,
    pub release_digest: ContentHash,
    pub provenance_digest: ContentHash,
    pub replay_identity: ContentHash,
    pub benchmark_digest: ContentHash,
    pub raw_data_local: bool,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReleaseAssuranceReceipt {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub workflow_id: String,
    pub scope: String,
    pub disposition: ReleaseDisposition,
    pub candidate_order: Vec<String>,
    pub admitted_order: Vec<String>,
    pub blocked_order: Vec<String>,
    pub unknown_order: Vec<String>,
    pub release_order: Vec<String>,
    pub artifact_order: Vec<String>,
    pub evidence_order: Vec<String>,
    pub provenance_order: Vec<ContentHash>,
    pub replay_order: Vec<ContentHash>,
    pub benchmark_order: Vec<ContentHash>,
    pub omissions: Vec<String>,
    pub uncertainty: Vec<String>,
    pub negative_evidence: Vec<String>,
    pub replay_identity: ContentHash,
    pub benchmark_digest: Option<ContentHash>,
    pub effect_receipts: Vec<String>,
    pub objects: Vec<SignedResearchObject>,
    pub artifact: TypedResearchArtifact,
    pub raw_data_local: bool,
    pub boundary: String,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ReleaseAssuranceError {
    #[error("invalid release assurance request: {0}")]
    Invalid(String),
    #[error("release assurance artifact failed: {0}")]
    Artifact(String),
    #[error("release assurance serialization failed: {0}")]
    Serialization(String),
}

impl ReleaseAssuranceReceipt {
    pub fn validate(&self) -> Result<(), ReleaseAssuranceError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.boundary != PRECLINICAL_BOUNDARY
            || !self.raw_data_local
            || self.request_id.trim().is_empty()
            || self.workflow_id.trim().is_empty()
            || self.scope.trim().is_empty()
            || self.candidate_order.is_empty()
            || self.objects.len() > self.admitted_order.len()
            || (self.effect_receipts.is_empty()
                && self.disposition != ReleaseDisposition::Qualified)
        {
            return Err(ReleaseAssuranceError::Invalid("release assurance identity, candidates, locality, effects, or boundary is incomplete".into()));
        }
        if self
            .admitted_order
            .iter()
            .any(|id| !self.candidate_order.contains(id))
            || self
                .blocked_order
                .iter()
                .any(|id| !self.candidate_order.contains(id))
            || self
                .unknown_order
                .iter()
                .any(|id| !self.candidate_order.contains(id))
        {
            return Err(ReleaseAssuranceError::Invalid(
                "release candidate state is not covered by candidate order".into(),
            ));
        }
        for values in [
            &self.candidate_order,
            &self.admitted_order,
            &self.blocked_order,
            &self.unknown_order,
            &self.release_order,
            &self.artifact_order,
            &self.evidence_order,
            &self.omissions,
            &self.uncertainty,
            &self.negative_evidence,
            &self.effect_receipts,
        ] {
            if values.windows(2).any(|pair| pair[0] >= pair[1]) {
                return Err(ReleaseAssuranceError::Invalid(
                    "release assurance ordering is not canonical".into(),
                ));
            }
        }
        for values in [
            &self.provenance_order,
            &self.replay_order,
            &self.benchmark_order,
        ] {
            if values.windows(2).any(|pair| pair[0] >= pair[1]) {
                return Err(ReleaseAssuranceError::Invalid(
                    "release assurance digest ordering is not canonical".into(),
                ));
            }
        }
        if self.effect_receipts.iter().any(|effect| {
            effect != "block:unsafe-release" && !effect.starts_with("evaluate:release-assurance:")
        }) {
            return Err(ReleaseAssuranceError::Invalid(
                "release assurance effect is outside the release gate".into(),
            ));
        }
        for object in &self.objects {
            if object.boundary != PRECLINICAL_BOUNDARY
                || !object.raw_data_local
                || object.artifact_ids.is_empty()
                || object.evidence_receipt_ids.is_empty()
            {
                return Err(ReleaseAssuranceError::Invalid(
                    "signed research object is incomplete or non-local".into(),
                ));
            }
        }
        self.artifact
            .validate_metadata()
            .map_err(|error| ReleaseAssuranceError::Artifact(error.to_string()))
    }

    pub fn digest(&self) -> Result<ContentHash, ReleaseAssuranceError> {
        self.validate()?;
        let value = serde_json::to_value(self)
            .map_err(|error| ReleaseAssuranceError::Serialization(error.to_string()))?;
        ContentHash::of_value(&value)
            .map_err(|error| ReleaseAssuranceError::Serialization(error.to_string()))
    }
}

pub fn release_assurance_manifest() -> CapabilityManifest {
    CapabilityManifest {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        capability_id: FEATURE_ID.into(),
        version: CONTRACT_VERSION.into(),
        owner_crate: "api".into(),
        consumers: ["preclinical researcher".into(), "publication release service".into(), "conformance operator".into()].into(),
        behavior: "verifies prospective high-throughput validated research runs before release, preserving evidence, provenance, replay, benchmark, omission, negative-result, and locality witnesses without signing or exporting data".into(),
        value: "provides a separately versioned API release gate that can be replayed by independent conformance systems and cannot silently pass incomplete research objects".into(),
        inputs: vec![TypedPort { name: "validated_research_run_batch".into(), schema: "ValidatedResearchRun3@1".into(), required: true }],
        outputs: vec![TypedPort { name: "signed_research_object_batch".into(), schema: "SignedResearchObject7@1".into(), required: true }],
        effects: [Effect::ReadLocalData, Effect::ExecuteLocalComputation, Effect::WriteLocalArtifact].into(),
        permissions: ["evaluate:capability-runs".into(), "write:local-release-assurance".into()].into(),
        determinism: Determinism::ByteStable,
        evidence: vec![
            EvidenceReference { source_id: "w3c-prov-o".into(), state: EvidenceState::Supported, locator: Some("https://www.w3.org/TR/prov-o/".into()) },
            EvidenceReference { source_id: "ro-crate-1.3".into(), state: EvidenceState::Supported, locator: Some("https://www.researchobject.org/ro-crate/specification.html".into()) },
            EvidenceReference { source_id: "ga4gh-drs-1.3".into(), state: EvidenceState::Supported, locator: Some("https://ga4gh.github.io/data-repository-service-schemas/preview/release/drs-1.3.0/docs/".into()) },
        ],
        authority_requirements: vec![AuthorityRequirement { role: "release conformance approver".into(), reason: "approve the release gate configuration before publication service consumption".into() }],
        autonomy_tier: AutonomyTier::A1,
        surfaces: [ResearchSurface::Ui, ResearchSurface::Api, ResearchSurface::Sdk, ResearchSurface::Cli, ResearchSurface::McpTool, ResearchSurface::Policy, ResearchSurface::Operator].into(),
        boundary: PRECLINICAL_BOUNDARY.into(),
    }
}

pub fn assure_release(
    request: &ReleaseAssuranceRequest,
) -> Result<ReleaseAssuranceReceipt, ReleaseAssuranceError> {
    validate_request(request)?;
    let mut candidates = request.candidates.clone();
    candidates.sort_by(|left, right| {
        left.release_id
            .cmp(&right.release_id)
            .then(left.run_id.cmp(&right.run_id))
    });
    let candidate_order = candidates
        .iter()
        .map(|candidate| candidate.release_id.clone())
        .collect::<Vec<_>>();
    let mut admitted = Vec::new();
    let mut blocked = BTreeSet::new();
    let mut unknown = BTreeSet::new();
    let mut releases = BTreeSet::new();
    let mut artifacts = BTreeSet::new();
    let mut evidence = BTreeSet::new();
    let mut provenance = BTreeSet::new();
    let mut replay = BTreeSet::new();
    let mut benchmarks = BTreeSet::new();
    let mut omissions = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    let mut negative = BTreeSet::new();
    let mut objects = Vec::new();
    let mut spent = 0_u64;
    for candidate in &candidates {
        let cost = (candidate.run_id.len()
            + candidate.release_id.len()
            + candidate.artifact_ids.len()
            + candidate.evidence_receipt_ids.len()) as u64
            + 1;
        let budget_ok = cost <= request.budget.saturating_sub(spent);
        let complete = candidate.state == ReleaseState::Supported
            && candidate.scope == request.scope
            && !candidate.artifact_ids.is_empty()
            && !candidate.evidence_receipt_ids.is_empty()
            && candidate.benchmark_digest.is_some()
            && request.benchmark_digest.is_some()
            && candidate.replay_identity == request.replay_identity
            && candidate.provenance_digest != ContentHash::of_bytes(b"")
            && candidate.omissions.is_empty()
            && candidate.uncertainty.is_empty()
            && candidate.negative_evidence.is_empty()
            && candidate.raw_data_local
            && request.raw_data_local
            && request.policy_allow
            && request.protected_closure
            && request.signed_approval
            && budget_ok;
        if complete && admitted.len() < request.max_admissions {
            spent = spent.saturating_add(cost);
            admitted.push(candidate.release_id.clone());
            releases.insert(candidate.release_id.clone());
            artifacts.extend(candidate.artifact_ids.iter().cloned());
            evidence.extend(candidate.evidence_receipt_ids.iter().cloned());
            provenance.insert(candidate.provenance_digest.clone());
            replay.insert(candidate.replay_identity.clone());
            if let Some(digest) = &candidate.benchmark_digest {
                benchmarks.insert(digest.clone());
            }
            objects.push(SignedResearchObject {
                run_id: candidate.run_id.clone(),
                release_id: candidate.release_id.clone(),
                artifact_ids: candidate.artifact_ids.clone(),
                evidence_receipt_ids: candidate.evidence_receipt_ids.clone(),
                release_digest: candidate.release_digest.clone(),
                provenance_digest: candidate.provenance_digest.clone(),
                replay_identity: candidate.replay_identity.clone(),
                benchmark_digest: candidate.benchmark_digest.clone().expect("checked above"),
                raw_data_local: true,
                boundary: PRECLINICAL_BOUNDARY.into(),
            });
        } else {
            blocked.insert(candidate.release_id.clone());
            if matches!(
                candidate.state,
                ReleaseState::Unknown | ReleaseState::Unmeasured
            ) {
                unknown.insert(candidate.release_id.clone());
                uncertainty.insert(
                    format!(
                        "release:{}:state-{:?}-not-admitted",
                        candidate.release_id, candidate.state
                    )
                    .to_ascii_lowercase(),
                );
            }
            if candidate.state == ReleaseState::Contradicted {
                negative.insert(format!(
                    "release:{}:contradicted-negative-evidence",
                    candidate.release_id
                ));
            }
            if candidate.scope != request.scope {
                omissions.insert(format!("release:{}:scope-mismatch", candidate.release_id));
            }
            if candidate.artifact_ids.is_empty() {
                omissions.insert(format!("release:{}:artifact-missing", candidate.release_id));
            }
            if candidate.evidence_receipt_ids.is_empty() {
                omissions.insert(format!("release:{}:evidence-missing", candidate.release_id));
            }
            if candidate.benchmark_digest.is_none() || request.benchmark_digest.is_none() {
                omissions.insert(format!(
                    "release:{}:benchmark-missing",
                    candidate.release_id
                ));
            }
            if candidate.replay_identity != request.replay_identity {
                uncertainty.insert(format!("release:{}:replay-mismatch", candidate.release_id));
            }
            if !candidate.omissions.is_empty() {
                uncertainty.insert(format!(
                    "release:{}:protected-closure-incomplete",
                    candidate.release_id
                ));
            }
            if !candidate.uncertainty.is_empty() {
                uncertainty.insert(format!(
                    "release:{}:uncertainty-unresolved",
                    candidate.release_id
                ));
            }
            if !candidate.negative_evidence.is_empty() {
                negative.insert(format!(
                    "release:{}:negative-evidence-present",
                    candidate.release_id
                ));
            }
            if !request.policy_allow {
                negative.insert("request:policy-denied".into());
            }
            if !request.protected_closure {
                uncertainty.insert("request:protected-closure-incomplete".into());
            }
            if !request.signed_approval {
                omissions.insert("request:signed-approval-required".into());
            }
            if !request.raw_data_local || !candidate.raw_data_local {
                negative.insert(format!(
                    "release:{}:raw-data-locality-failed",
                    candidate.release_id
                ));
            }
            if !budget_ok {
                omissions.insert(format!("release:{}:budget-exhausted", candidate.release_id));
            }
            if admitted.len() >= request.max_admissions {
                omissions.insert(format!("release:{}:admission-limit", candidate.release_id));
            }
        }
    }
    if request.benchmark_digest.is_none() {
        uncertainty.insert("request:benchmark-missing".into());
    }
    let disposition = if !request.policy_allow
        || !request.protected_closure
        || !request.signed_approval
        || !request.raw_data_local
    {
        ReleaseDisposition::Blocked
    } else if admitted.is_empty() {
        ReleaseDisposition::Unknown
    } else if blocked.is_empty()
        && omissions.is_empty()
        && uncertainty.is_empty()
        && negative.is_empty()
    {
        ReleaseDisposition::Qualified
    } else {
        ReleaseDisposition::Partial
    };
    let payload = json!({"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "contract_version": CONTRACT_VERSION, "feature_id": FEATURE_ID, "request_id": request.request_id, "workflow_id": request.workflow_id, "scope": request.scope, "disposition": disposition, "candidate_order": candidate_order, "admitted_order": admitted, "blocked_order": blocked, "unknown_order": unknown, "release_order": releases, "artifact_order": artifacts, "evidence_order": evidence, "omissions": omissions, "uncertainty": uncertainty, "negative_evidence": negative, "replay_identity": request.replay_identity, "benchmark_digest": request.benchmark_digest, "boundary": PRECLINICAL_BOUNDARY});
    let artifact = TypedResearchArtifact::from_payload(
        format!("release-assurance:{}", request.request_id),
        "application/vnd.aurora.release-assurance+json",
        &payload,
        Vec::new(),
        Vec::new(),
    )
    .map_err(|error| ReleaseAssuranceError::Artifact(error.to_string()))?;
    let effect_receipts = if admitted.is_empty() {
        vec!["block:unsafe-release".into()]
    } else {
        vec![format!("evaluate:release-assurance:{}", request.request_id)]
    };
    let receipt = ReleaseAssuranceReceipt {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        request_id: request.request_id.clone(),
        workflow_id: request.workflow_id.clone(),
        scope: request.scope.clone(),
        disposition,
        candidate_order,
        admitted_order: admitted,
        blocked_order: blocked.into_iter().collect(),
        unknown_order: unknown.into_iter().collect(),
        release_order: releases.into_iter().collect(),
        artifact_order: artifacts.into_iter().collect(),
        evidence_order: evidence.into_iter().collect(),
        provenance_order: provenance.into_iter().collect(),
        replay_order: replay.into_iter().collect(),
        benchmark_order: benchmarks.into_iter().collect(),
        omissions: omissions.into_iter().collect(),
        uncertainty: uncertainty.into_iter().collect(),
        negative_evidence: negative.into_iter().collect(),
        replay_identity: request.replay_identity.clone(),
        benchmark_digest: request.benchmark_digest.clone(),
        effect_receipts,
        objects,
        artifact,
        raw_data_local: true,
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    receipt.validate()?;
    Ok(receipt)
}

fn validate_request(request: &ReleaseAssuranceRequest) -> Result<(), ReleaseAssuranceError> {
    if request.request_id.trim().is_empty()
        || request.workflow_id.trim().is_empty()
        || request.scope.trim().is_empty()
        || request.candidates.is_empty()
        || request.candidates.len() > MAX_CANDIDATES
        || request.max_admissions == 0
        || request.budget == 0
        || request.boundary != PRECLINICAL_BOUNDARY
    {
        return Err(ReleaseAssuranceError::Invalid(
            "release request identity, candidates, limits, budget, or boundary is incomplete"
                .into(),
        ));
    }
    let mut ids = BTreeSet::new();
    for candidate in &request.candidates {
        if candidate.run_id.trim().is_empty()
            || candidate.release_id.trim().is_empty()
            || candidate.scope.trim().is_empty()
            || candidate.boundary != PRECLINICAL_BOUNDARY
            || !ids.insert(candidate.release_id.clone())
        {
            return Err(ReleaseAssuranceError::Invalid(format!(
                "release {} is invalid or duplicated",
                candidate.release_id
            )));
        }
        unique(&candidate.artifact_ids)?;
        unique(&candidate.evidence_receipt_ids)?;
    }
    Ok(())
}

fn unique(values: &[String]) -> Result<(), ReleaseAssuranceError> {
    let mut seen = BTreeSet::new();
    if values
        .iter()
        .any(|value| value.trim().is_empty() || !seen.insert(value))
    {
        return Err(ReleaseAssuranceError::Invalid(
            "release identifiers are empty or duplicated".into(),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn hash(value: &str) -> ContentHash {
        ContentHash::of_bytes(value.as_bytes())
    }
    fn candidate(id: &str, state: ReleaseState) -> ValidatedResearchRun {
        ValidatedResearchRun {
            run_id: format!("run:{id}"),
            release_id: format!("release:{id}"),
            scope: "organoid:neural".into(),
            artifact_ids: vec![format!("artifact:{id}")],
            evidence_receipt_ids: vec![format!("evidence:{id}")],
            release_digest: hash(&format!("release:{id}")),
            provenance_digest: hash(&format!("provenance:{id}")),
            replay_identity: hash("replay"),
            benchmark_digest: Some(hash("benchmark")),
            state,
            omissions: Vec::new(),
            uncertainty: Vec::new(),
            negative_evidence: Vec::new(),
            raw_data_local: true,
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }
    fn request(candidates: Vec<ValidatedResearchRun>) -> ReleaseAssuranceRequest {
        ReleaseAssuranceRequest {
            request_id: "request:release".into(),
            workflow_id: "workflow:publication".into(),
            scope: "organoid:neural".into(),
            candidates,
            replay_identity: hash("replay"),
            benchmark_digest: Some(hash("benchmark")),
            max_admissions: 4,
            budget: 10_000,
            policy_allow: true,
            protected_closure: true,
            signed_approval: true,
            raw_data_local: true,
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }
    #[test]
    fn manifest_is_typed_a1() {
        let manifest = release_assurance_manifest();
        manifest.validate().unwrap();
        assert_eq!(manifest.autonomy_tier, AutonomyTier::A1);
    }
    #[test]
    fn supported_release_is_qualified_and_deterministic() {
        let receipt = assure_release(&request(vec![
            candidate("b", ReleaseState::Supported),
            candidate("a", ReleaseState::Supported),
        ]))
        .unwrap();
        assert_eq!(receipt.disposition, ReleaseDisposition::Qualified);
        assert_eq!(receipt.candidate_order, vec!["release:a", "release:b"]);
        assert_eq!(receipt.digest().unwrap(), receipt.digest().unwrap());
    }
    #[test]
    fn unknown_and_contradicted_releases_remain_visible() {
        let receipt = assure_release(&request(vec![
            candidate("a", ReleaseState::Supported),
            candidate("b", ReleaseState::Unknown),
            candidate("c", ReleaseState::Contradicted),
        ]))
        .unwrap();
        assert_eq!(receipt.disposition, ReleaseDisposition::Partial);
        assert!(receipt.unknown_order.contains(&"release:b".into()));
        assert!(receipt
            .negative_evidence
            .iter()
            .any(|item| item.contains("release:c")));
    }
    #[test]
    fn missing_benchmark_is_unknown() {
        let mut input = request(vec![candidate("a", ReleaseState::Supported)]);
        input.candidates[0].benchmark_digest = None;
        let receipt = assure_release(&input).unwrap();
        assert_eq!(receipt.disposition, ReleaseDisposition::Unknown);
        assert!(receipt
            .omissions
            .iter()
            .any(|item| item.contains("benchmark-missing")));
    }
    #[test]
    fn policy_denial_blocks_release() {
        let mut input = request(vec![candidate("a", ReleaseState::Supported)]);
        input.policy_allow = false;
        let receipt = assure_release(&input).unwrap();
        assert_eq!(receipt.disposition, ReleaseDisposition::Blocked);
        assert_eq!(receipt.effect_receipts, vec!["block:unsafe-release"]);
    }
    #[test]
    fn duplicate_release_is_rejected() {
        let mut duplicate = candidate("a", ReleaseState::Supported);
        duplicate.run_id = "run:other".into();
        assert!(assure_release(&request(vec![
            candidate("a", ReleaseState::Supported),
            duplicate
        ]))
        .is_err());
    }
}
