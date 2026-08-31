//! Prospective high-throughput publication and research-object release contract.
//!
//! Atlas feature `AFA-oraclex-P16-F07`.  This is a typed admission boundary for a release batch,
//! not a publication generator: it decides whether caller-supplied research objects have enough
//! provenance, evaluation, evidence, reproducibility, policy and authority material to enter a
//! signed research-object channel.  Payloads remain institution-local; the emitted artifact is a
//! portable receipt and digest-only release index.

use std::collections::BTreeSet;

use bioprism_foundation::{
    AuthorityRequirement, AutonomyTier, CapabilityManifest, Determinism, Effect, EvidenceReference,
    EvidenceState, LossSeverity, ProvenanceLink, ResearchSurface, SemanticLoss, TypedPort,
    TypedResearchArtifact, PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION,
};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;
use thiserror::Error;

pub fn feature_id() -> &'static str {
    "AFA-oraclex-P16-F07"
}

pub fn contract_version() -> &'static str {
    "oraclex-prospective-publication-release/1.0"
}

pub fn input_schema() -> &'static str {
    "PublicationReleaseBatch1@1"
}

pub fn output_schema() -> &'static str {
    "PublicationReleaseReceipt1@1"
}

/// A single research object proposed for release.  It carries only content-addressed joins; the
/// corresponding payload and raw observations stay at the originating institution.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReleaseCandidate {
    pub artifact_id: String,
    pub title: String,
    pub version: String,
    pub artifact_digest: ContentHash,
    pub provenance_digest: ContentHash,
    pub workflow_digest: ContentHash,
    pub evidence_digest: ContentHash,
    pub source_digests: Vec<ContentHash>,
    pub schema_version: String,
    pub standards: BTreeSet<String>,
    pub evidence_state: EvidenceState,
    pub negative_findings: Vec<String>,
    pub replication_sites: BTreeSet<String>,
    pub baseline_id: String,
    pub evaluation_digest: ContentHash,
    pub license: String,
    pub raw_data_local: bool,
}

/// Admission inputs for a prospective high-throughput release batch.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PublicationReleaseRequest {
    pub request_id: String,
    pub consumer: String,
    pub batch_id: String,
    pub release_channel: String,
    pub capacity: u32,
    pub active_jobs: u32,
    pub candidates: Vec<ReleaseCandidate>,
    pub required_standards: BTreeSet<String>,
    pub policy_allow: bool,
    pub protected_closure: bool,
    pub signed_approval: bool,
    pub approval_token: String,
    pub reproducibility_bundle: bool,
    pub all_negative_results_reported: bool,
    pub network_permitted: bool,
    pub raw_data_local: bool,
    pub boundary: String,
    pub replay_identity: ContentHash,
    pub prior_release_digest: Option<ContentHash>,
    pub migration_from: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReleaseCandidateDecision {
    pub artifact_id: String,
    pub disposition: String,
    pub failed_gates: Vec<String>,
    pub conditional_gates: Vec<String>,
    pub negative_findings: Vec<String>,
    pub source_digests: Vec<ContentHash>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PublicationReleaseReceipt {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub consumer: String,
    pub batch_id: String,
    pub release_channel: String,
    pub verdict: String,
    pub candidate_order: Vec<String>,
    pub accepted_order: Vec<String>,
    pub conditional_order: Vec<String>,
    pub blocked_order: Vec<String>,
    pub unknown_order: Vec<String>,
    pub decisions: Vec<ReleaseCandidateDecision>,
    pub gate_order: Vec<String>,
    pub passed_gates: Vec<String>,
    pub failed_gates: Vec<String>,
    pub omissions: Vec<String>,
    pub uncertainty: Vec<String>,
    pub negative_evidence: Vec<String>,
    pub semantic_loss: Vec<SemanticLoss>,
    pub replay_identity: ContentHash,
    pub prior_release_digest: Option<ContentHash>,
    pub release_digest: ContentHash,
    pub effect_receipts: Vec<String>,
    pub artifact: TypedResearchArtifact,
    pub raw_data_local: bool,
    pub network_permitted: bool,
    pub boundary: String,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum PublicationReleaseError {
    #[error("invalid publication release request: {0}")]
    Invalid(String),
    #[error("publication release artifact failed: {0}")]
    Artifact(String),
}

fn digest_is_well_formed(value: &ContentHash) -> bool {
    value.as_str().len() == 64
}

fn canonical(values: &[String]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn required_candidate_fields(candidate: &ReleaseCandidate) -> bool {
    !candidate.artifact_id.trim().is_empty()
        && !candidate.title.trim().is_empty()
        && !candidate.version.trim().is_empty()
        && !candidate.schema_version.trim().is_empty()
        && !candidate.baseline_id.trim().is_empty()
        && !candidate.license.trim().is_empty()
        && candidate.raw_data_local
        && !candidate.source_digests.is_empty()
        && candidate.source_digests.iter().all(digest_is_well_formed)
        && digest_is_well_formed(&candidate.artifact_digest)
        && digest_is_well_formed(&candidate.provenance_digest)
        && digest_is_well_formed(&candidate.workflow_digest)
        && digest_is_well_formed(&candidate.evidence_digest)
        && digest_is_well_formed(&candidate.evaluation_digest)
}

impl PublicationReleaseReceipt {
    pub fn validate(&self) -> Result<(), PublicationReleaseError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.contract_version != contract_version()
            || self.feature_id != feature_id()
            || self.boundary != PRECLINICAL_BOUNDARY
            || !self.raw_data_local
            || self.request_id.trim().is_empty()
            || self.consumer.trim().is_empty()
            || self.batch_id.trim().is_empty()
            || self.release_channel.trim().is_empty()
            || self.candidate_order.is_empty()
            || !matches!(
                self.verdict.as_str(),
                "released" | "conditional" | "blocked" | "unknown"
            )
            || self.effect_receipts.is_empty()
            || !digest_is_well_formed(&self.replay_identity)
            || !digest_is_well_formed(&self.release_digest)
        {
            return Err(Self::invalid("release receipt identity, verdict, locality, candidates, effects, or digest is incomplete"));
        }
        for values in [
            &self.candidate_order,
            &self.accepted_order,
            &self.conditional_order,
            &self.blocked_order,
            &self.unknown_order,
            &self.gate_order,
            &self.passed_gates,
            &self.failed_gates,
            &self.omissions,
            &self.uncertainty,
            &self.negative_evidence,
        ] {
            if !canonical(values) {
                return Err(Self::invalid("release receipt ordering is not canonical"));
            }
        }
        if self.decisions.len() != self.candidate_order.len()
            || self
                .decisions
                .iter()
                .map(|decision| decision.artifact_id.as_str())
                .collect::<Vec<_>>()
                != self
                    .candidate_order
                    .iter()
                    .map(String::as_str)
                    .collect::<Vec<_>>()
        {
            return Err(Self::invalid(
                "release decisions do not match candidate order",
            ));
        }
        if self
            .accepted_order
            .iter()
            .chain(self.conditional_order.iter())
            .chain(self.blocked_order.iter())
            .chain(self.unknown_order.iter())
            .any(|id| !self.candidate_order.contains(id))
        {
            return Err(Self::invalid(
                "release disposition references an unknown candidate",
            ));
        }
        if self.effect_receipts.iter().any(|effect| {
            effect != "release:publication-research-object"
                && effect != "release:conditional-review"
                && effect != "block:unsafe-release"
        }) {
            return Err(Self::invalid(
                "release effect is outside the publication gate",
            ));
        }
        if self.verdict == "released"
            && self.effect_receipts != ["release:publication-research-object"]
        {
            return Err(Self::invalid(
                "released receipt must contain only the release effect",
            ));
        }
        if self.verdict != "released"
            && self.effect_receipts == ["release:publication-research-object"]
        {
            return Err(Self::invalid(
                "non-released receipt cannot claim a release effect",
            ));
        }
        self.artifact
            .validate_metadata()
            .map_err(|error| PublicationReleaseError::Artifact(error.to_string()))
    }

    fn invalid(message: &str) -> PublicationReleaseError {
        PublicationReleaseError::Invalid(message.into())
    }
}

pub fn publication_release_contract_manifest() -> CapabilityManifest {
    CapabilityManifest {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        capability_id: feature_id().into(),
        version: contract_version().into(),
        owner_crate: "oraclex".into(),
        consumers: [
            "research program lead".into(),
            "publication steward".into(),
            "federated repository".into(),
        ]
        .into(),
        behavior: "admits a prospective release batch only when typed research objects satisfy provenance, evaluation, evidence, reproducibility, policy, authority, locality, and migration gates".into(),
        value: "prevents unsupported or irreproducible preclinical research objects from being published while preserving uncertainty, negative findings, omissions, and digest-only locality".into(),
        inputs: vec![TypedPort { name: "publication_release_batch".into(), schema: input_schema().into(), required: true }],
        outputs: vec![TypedPort { name: "publication_release_receipt".into(), schema: output_schema().into(), required: true }],
        effects: [Effect::ReadLocalData, Effect::WriteLocalArtifact, Effect::FederationExport].into(),
        permissions: ["release:publication-research-object".into()].into(),
        determinism: Determinism::ByteStable,
        evidence: vec![
            EvidenceReference { source_id: "ro-crate".into(), state: EvidenceState::Supported, locator: Some("https://www.researchobject.org/ro-crate/specification.html".into()) },
            EvidenceReference { source_id: "w3c-prov-o".into(), state: EvidenceState::Supported, locator: Some("https://www.w3.org/TR/prov-o/".into()) },
            EvidenceReference { source_id: "slsa".into(), state: EvidenceState::Supported, locator: Some("https://slsa.dev/spec/v1.2/provenance".into()) },
            EvidenceReference { source_id: "sigstore".into(), state: EvidenceState::Supported, locator: Some("https://docs.sigstore.dev/".into()) },
        ],
        authority_requirements: vec![AuthorityRequirement { role: "publication steward".into(), reason: "external release changes the durable research commons".into() }],
        autonomy_tier: AutonomyTier::A2,
        surfaces: [ResearchSurface::Ui, ResearchSurface::Cli, ResearchSurface::Api, ResearchSurface::Sdk, ResearchSurface::McpTool, ResearchSurface::Protocol, ResearchSurface::Policy, ResearchSurface::Operator].into(),
        boundary: PRECLINICAL_BOUNDARY.into(),
    }
}

pub fn compile_publication_release(
    request: &PublicationReleaseRequest,
) -> Result<PublicationReleaseReceipt, PublicationReleaseError> {
    if request.request_id.trim().is_empty()
        || request.consumer.trim().is_empty()
        || request.batch_id.trim().is_empty()
        || request.release_channel.trim().is_empty()
        || request.capacity == 0
        || request.active_jobs > request.capacity
        || request.candidates.is_empty()
        || request.candidates.len() as u32 > request.capacity
        || request.required_standards.is_empty()
        || request.approval_token.trim().is_empty() && request.signed_approval
        || !request.raw_data_local
        || request.boundary != PRECLINICAL_BOUNDARY
        || !digest_is_well_formed(&request.replay_identity)
        || request
            .required_standards
            .iter()
            .any(|standard| standard.trim().is_empty())
    {
        return Err(PublicationReleaseError::Invalid("release request identity, capacity, standards, locality, approval, replay, or boundary is invalid".into()));
    }

    let mut candidates = request.candidates.clone();
    candidates.sort_by(|left, right| left.artifact_id.cmp(&right.artifact_id));
    if candidates
        .windows(2)
        .any(|pair| pair[0].artifact_id == pair[1].artifact_id)
        || candidates
            .iter()
            .any(|candidate| !required_candidate_fields(candidate))
    {
        return Err(PublicationReleaseError::Invalid(
            "release candidates must have unique ids and complete local digest envelopes".into(),
        ));
    }

    let mut candidate_order = Vec::new();
    let mut accepted_order = Vec::new();
    let mut conditional_order = Vec::new();
    let mut blocked_order = Vec::new();
    let mut unknown_order = Vec::new();
    let mut decisions = Vec::new();
    let mut negative_evidence = BTreeSet::new();
    let mut omissions = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    let mut global_failed: BTreeSet<String> = BTreeSet::new();

    let required_standards: Vec<String> = request.required_standards.iter().cloned().collect();
    for candidate in &candidates {
        candidate_order.push(candidate.artifact_id.clone());
        let mut failed: BTreeSet<String> = BTreeSet::new();
        let mut conditional: BTreeSet<String> = BTreeSet::new();
        if !request.policy_allow {
            failed.insert("policy-allow".into());
        }
        if !request.protected_closure {
            failed.insert("protected-closure".into());
        }
        if !request.signed_approval {
            failed.insert("signed-approval".into());
        }
        if !request.reproducibility_bundle {
            failed.insert("reproducibility-bundle".into());
        }
        if !request.all_negative_results_reported {
            failed.insert("negative-result-disclosure".into());
        }
        if !request.network_permitted {
            failed.insert("federation-permission".into());
        }
        if candidate.standards.is_disjoint(&request.required_standards) {
            failed.insert("standards-coverage".into());
        } else if required_standards
            .iter()
            .any(|standard| !candidate.standards.contains(standard))
        {
            conditional.insert("partial-standards-coverage".into());
        }
        if candidate.replication_sites.is_empty() {
            conditional.insert("replication-site".into());
        }
        if candidate.evidence_state == EvidenceState::Contradicted {
            failed.insert("contradicted-evidence".into());
        }
        if matches!(
            candidate.evidence_state,
            EvidenceState::Unknown | EvidenceState::Speculative
        ) {
            conditional.insert("evidence-state".into());
            uncertainty.insert(format!("{}:evidence-state", candidate.artifact_id));
        }
        if candidate.negative_findings.is_empty() {
            omissions.insert(format!(
                "{}:negative-findings-not-observed",
                candidate.artifact_id
            ));
        } else {
            for finding in &candidate.negative_findings {
                negative_evidence.insert(format!("{}:{finding}", candidate.artifact_id));
            }
        }
        for gate in &failed {
            global_failed.insert(gate.clone());
        }
        let disposition = if !failed.is_empty() {
            blocked_order.push(candidate.artifact_id.clone());
            "blocked"
        } else if !conditional.is_empty() {
            conditional_order.push(candidate.artifact_id.clone());
            "conditional"
        } else {
            accepted_order.push(candidate.artifact_id.clone());
            "accepted"
        };
        if disposition == "unknown" {
            unknown_order.push(candidate.artifact_id.clone());
        }
        decisions.push(ReleaseCandidateDecision {
            artifact_id: candidate.artifact_id.clone(),
            disposition: disposition.into(),
            failed_gates: failed.into_iter().collect(),
            conditional_gates: conditional.into_iter().collect(),
            negative_findings: candidate.negative_findings.clone(),
            source_digests: candidate.source_digests.clone(),
        });
    }

    if request.active_jobs >= request.capacity {
        global_failed.insert("capacity".into());
    }
    if let Some(migration) = &request.migration_from {
        if migration != contract_version() {
            uncertainty.insert(format!("migration:{migration}"));
        }
    }
    let semantic_loss = if request.migration_from.is_some() {
        vec![SemanticLoss { field: "contract_version".into(), reason: "release receipt was compiled across a version boundary; recipient must replay the pinned source contract".into(), severity: LossSeverity::Bounded }]
    } else {
        Vec::new()
    };
    let verdict = if !global_failed.is_empty() || !blocked_order.is_empty() {
        "blocked"
    } else if !conditional_order.is_empty() {
        "conditional"
    } else if !accepted_order.is_empty() {
        "released"
    } else {
        "unknown"
    };
    let mut passed_gates: Vec<String> = [
        "typed-digests",
        "provenance",
        "workflow-replay",
        "evaluation-baseline",
        "raw-locality",
    ]
    .iter()
    .filter(|gate| !global_failed.contains(**gate))
    .map(|gate| (*gate).to_string())
    .collect();
    passed_gates.sort();
    let mut gate_order = [
        "capacity",
        "evaluation-baseline",
        "federation-permission",
        "negative-result-disclosure",
        "policy-allow",
        "provenance",
        "protected-closure",
        "reproducibility-bundle",
        "signed-approval",
        "standards-coverage",
        "typed-digests",
        "workflow-replay",
    ]
    .iter()
    .map(|gate| (*gate).to_string())
    .collect::<Vec<_>>();
    gate_order.sort();
    let failed_gates: Vec<String> = global_failed.into_iter().collect();
    let effect_receipts = match verdict {
        "released" => vec!["release:publication-research-object".into()],
        "conditional" => vec![
            "release:conditional-review".into(),
            "block:unsafe-release".into(),
        ],
        _ => vec!["block:unsafe-release".into()],
    };
    let payload = json!({
        "feature_id": feature_id(), "request_id": request.request_id, "batch_id": request.batch_id,
        "release_channel": request.release_channel, "verdict": verdict, "candidate_order": candidate_order.clone(),
        "accepted_order": accepted_order, "conditional_order": conditional_order,
        "blocked_order": blocked_order, "unknown_order": unknown_order, "decisions": decisions.clone(),
        "gate_order": gate_order, "passed_gates": passed_gates, "failed_gates": failed_gates,
        "omissions": omissions, "uncertainty": uncertainty, "negative_evidence": negative_evidence,
        "semantic_loss": semantic_loss, "replay_identity": request.replay_identity,
        "prior_release_digest": request.prior_release_digest,
    });
    let release_digest = ContentHash::of_value(&payload)
        .map_err(|error| PublicationReleaseError::Invalid(error.to_string()))?;
    let artifact = TypedResearchArtifact::from_payload(
        format!("{}:publication-release", request.request_id),
        "application/vnd.aurora.publication-release+json",
        &payload,
        semantic_loss.clone(),
        vec![ProvenanceLink {
            source_id: request.batch_id.clone(),
            relation: "compiled-from-release-batch".into(),
            digest: release_digest.clone(),
        }],
    )
    .map_err(|error| PublicationReleaseError::Artifact(error.to_string()))?;
    let receipt = PublicationReleaseReceipt {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        contract_version: contract_version().into(),
        feature_id: feature_id().into(),
        request_id: request.request_id.clone(),
        consumer: request.consumer.clone(),
        batch_id: request.batch_id.clone(),
        release_channel: request.release_channel.clone(),
        verdict: verdict.into(),
        candidate_order,
        accepted_order: accepted_order.clone(),
        conditional_order: conditional_order.clone(),
        blocked_order: blocked_order.clone(),
        unknown_order: unknown_order.clone(),
        decisions,
        gate_order: gate_order.clone(),
        passed_gates: passed_gates.clone(),
        failed_gates: failed_gates.clone(),
        omissions: omissions.into_iter().collect(),
        uncertainty: uncertainty.into_iter().collect(),
        negative_evidence: negative_evidence.into_iter().collect(),
        semantic_loss,
        replay_identity: request.replay_identity.clone(),
        prior_release_digest: request.prior_release_digest.clone(),
        release_digest,
        effect_receipts,
        artifact,
        raw_data_local: request.raw_data_local,
        network_permitted: request.network_permitted,
        boundary: request.boundary.clone(),
    };
    receipt.validate()?;
    Ok(receipt)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hash(label: &str) -> ContentHash {
        ContentHash::of_bytes(label.as_bytes())
    }

    fn candidate(id: &str, state: EvidenceState) -> ReleaseCandidate {
        ReleaseCandidate {
            artifact_id: id.into(),
            title: format!("{id} title"),
            version: "v1".into(),
            artifact_digest: hash("artifact"),
            provenance_digest: hash("provenance"),
            workflow_digest: hash("workflow"),
            evidence_digest: hash("evidence"),
            source_digests: vec![hash("source")],
            schema_version: "ro-crate/1".into(),
            standards: [
                "ro-crate".into(),
                "prov-o".into(),
                "slsa".into(),
                "sigstore".into(),
            ]
            .into(),
            evidence_state: state,
            negative_findings: vec!["null-effect".into()],
            replication_sites: ["site-a".into()].into(),
            baseline_id: "baseline-v1".into(),
            evaluation_digest: hash("evaluation"),
            license: "CC-BY-4.0".into(),
            raw_data_local: true,
        }
    }

    fn request(candidates: Vec<ReleaseCandidate>) -> PublicationReleaseRequest {
        PublicationReleaseRequest {
            request_id: "request-1".into(),
            consumer: "research program lead".into(),
            batch_id: "batch-1".into(),
            release_channel: "consortium".into(),
            capacity: 8,
            active_jobs: 0,
            candidates,
            required_standards: [
                "ro-crate".into(),
                "prov-o".into(),
                "slsa".into(),
                "sigstore".into(),
            ]
            .into(),
            policy_allow: true,
            protected_closure: true,
            signed_approval: true,
            approval_token: "approval-1".into(),
            reproducibility_bundle: true,
            all_negative_results_reported: true,
            network_permitted: true,
            raw_data_local: true,
            boundary: PRECLINICAL_BOUNDARY.into(),
            replay_identity: hash("replay"),
            prior_release_digest: None,
            migration_from: None,
        }
    }

    #[test]
    fn deterministic_batch_is_released_and_retains_negative_findings() {
        let request = request(vec![
            candidate("z", EvidenceState::Proven),
            candidate("a", EvidenceState::Supported),
        ]);
        let first = compile_publication_release(&request).unwrap();
        let second = compile_publication_release(&request).unwrap();
        assert_eq!(first, second);
        assert_eq!(first.verdict, "released");
        assert_eq!(first.candidate_order, vec!["a", "z"]);
        assert_eq!(first.negative_evidence.len(), 2);
        assert!(first.validate().is_ok());
    }

    #[test]
    fn unknown_evidence_is_conditional_not_silent_support() {
        let receipt = compile_publication_release(&request(vec![candidate(
            "unknown",
            EvidenceState::Unknown,
        )]))
        .unwrap();
        assert_eq!(receipt.verdict, "conditional");
        assert!(receipt
            .uncertainty
            .iter()
            .any(|item| item.contains("evidence-state")));
        assert!(receipt
            .effect_receipts
            .contains(&"block:unsafe-release".into()));
    }

    #[test]
    fn policy_or_locality_failure_blocks_release() {
        let mut request = request(vec![candidate("blocked", EvidenceState::Proven)]);
        request.policy_allow = false;
        request.raw_data_local = false;
        let error = compile_publication_release(&request).unwrap_err();
        assert!(error.to_string().contains("locality"));
    }

    #[test]
    fn migration_is_explicitly_lossy_and_replay_bound() {
        let mut request = request(vec![candidate("migrated", EvidenceState::Supported)]);
        request.migration_from = Some("oraclex-prospective-publication-release/0.9".into());
        let receipt = compile_publication_release(&request).unwrap();
        assert_eq!(receipt.verdict, "released");
        assert_eq!(receipt.semantic_loss.len(), 1);
        assert_eq!(receipt.replay_identity, request.replay_identity);
    }

    #[test]
    fn manifest_is_a_valid_a2_contract() {
        assert!(publication_release_contract_manifest().validate().is_ok());
    }
}
