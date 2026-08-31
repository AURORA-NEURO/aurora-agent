//! Federated-continual publication and research-object release inference.
//!
//! Atlas feature `AFA-services-P16-F04`. This engine ranks release attestations and emits a
//! digest-only recommendation. It never signs, publishes, dereferences, or transports raw data;
//! those effects remain behind `research_release` and an explicit release authority.

use bioprism_foundation::{
    AuthorityRequirement, AutonomyTier, CapabilityManifest, Determinism, Effect, EvidenceReference,
    EvidenceState, LossSeverity, ProvenanceLink, ResearchSurface, SemanticLoss, TypedPort,
    TypedResearchArtifact, PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION,
};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::{BTreeSet, HashMap};
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-services-P16-F04";
pub const FEATURE_VERSION: &str = "services-federated-continual-publication-release-inference/1.0";
pub const INPUT_SCHEMA: &str = "FederatedPublicationReleaseInferenceBatch1@1";
pub const OUTPUT_SCHEMA: &str = "FederatedPublicationReleaseInferenceReceipt1@1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PublicationReleaseInferenceCandidate {
    pub release_id: String,
    pub origin: String,
    pub purpose: String,
    pub semantic_profile: String,
    pub artifact_digest: ContentHash,
    pub evidence_digest: ContentHash,
    pub provenance_digest: ContentHash,
    pub replay_identity: ContentHash,
    pub evidence_state: EvidenceState,
    pub freshness_seq: u64,
    pub omission_count: u32,
    pub negative_result: bool,
    pub policy_allow: bool,
    pub protected_closure: bool,
    pub signer_valid: bool,
    pub raw_data_local: bool,
    pub capability_complete: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederatedPublicationReleaseInferenceRequest {
    pub request_id: String,
    pub federation_id: String,
    pub purpose: String,
    pub semantic_profile: String,
    pub required_quorum: u32,
    pub capacity: u32,
    pub active_runs: u32,
    pub checkpoint_seq: u64,
    pub replay_identity: ContentHash,
    pub candidates: Vec<PublicationReleaseInferenceCandidate>,
    pub policy_allow: bool,
    pub protected_closure: bool,
    pub raw_data_local: bool,
    pub network_permitted: bool,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PublicationReleaseInferenceDecision {
    pub release_id: String,
    pub origin: String,
    pub score: i32,
    pub disposition: String,
    pub failed_gates: Vec<String>,
    pub conditional_gates: Vec<String>,
    pub negative_result: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederatedPublicationReleaseInferenceReceipt {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub federation_id: String,
    pub purpose: String,
    pub semantic_profile: String,
    pub admission: String,
    pub origin_order: Vec<String>,
    pub qualified_origin_order: Vec<String>,
    pub candidate_order: Vec<String>,
    pub rank_order: Vec<String>,
    pub qualified_order: Vec<String>,
    pub conditional_order: Vec<String>,
    pub blocked_order: Vec<String>,
    pub unknown_order: Vec<String>,
    pub decisions: Vec<PublicationReleaseInferenceDecision>,
    pub checkpoint_seq: u64,
    pub checkpoint_digest: ContentHash,
    pub inference_digest: ContentHash,
    pub replay_identity: ContentHash,
    pub semantic_loss: Vec<SemanticLoss>,
    pub omissions: Vec<String>,
    pub uncertainty: Vec<String>,
    pub negative_evidence: Vec<String>,
    pub effect_receipts: Vec<String>,
    pub artifact: TypedResearchArtifact,
    pub raw_data_local: bool,
    pub boundary: String,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum FederatedPublicationReleaseInferenceError {
    #[error("invalid publication-release inference request: {0}")]
    Invalid(String),
    #[error("publication-release inference artifact failed: {0}")]
    Artifact(String),
    #[error("publication-release inference serialization failed: {0}")]
    Serialization(String),
}

fn digest(value: &ContentHash) -> bool {
    value.as_str().len() == 64 && value.as_str().bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn canonical(values: &[String]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn unique_sorted(mut values: Vec<String>) -> Vec<String> {
    values.sort();
    values.dedup();
    values
}

fn evidence_score(state: EvidenceState) -> i32 {
    match state {
        EvidenceState::Proven => 40,
        EvidenceState::Supported => 30,
        EvidenceState::Speculative => 10,
        EvidenceState::Unknown => 0,
        EvidenceState::Contradicted => -40,
    }
}

impl FederatedPublicationReleaseInferenceReceipt {
    pub fn validate(&self) -> Result<(), FederatedPublicationReleaseInferenceError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.contract_version != FEATURE_VERSION
            || self.feature_id != FEATURE_ID
            || self.boundary != PRECLINICAL_BOUNDARY
            || !self.raw_data_local
            || self.request_id.trim().is_empty()
            || self.federation_id.trim().is_empty()
            || self.purpose.trim().is_empty()
            || self.semantic_profile.trim().is_empty()
            || self.candidate_order.is_empty()
            || self.decisions.len() != self.candidate_order.len()
            || self.effect_receipts.is_empty()
            || !matches!(
                self.admission.as_str(),
                "qualified" | "degraded" | "blocked" | "unknown"
            )
            || self.checkpoint_seq == 0
            || !digest(&self.checkpoint_digest)
            || !digest(&self.inference_digest)
            || !digest(&self.replay_identity)
        {
            return Err(Self::invalid(
                "inference identity, locality, admission, candidates, checkpoint, effects, or digests are incomplete",
            ));
        }
        for values in [
            &self.origin_order,
            &self.qualified_origin_order,
            &self.candidate_order,
            &self.qualified_order,
            &self.conditional_order,
            &self.blocked_order,
            &self.unknown_order,
            &self.omissions,
            &self.uncertainty,
            &self.negative_evidence,
            &self.effect_receipts,
        ] {
            if !canonical(values) {
                return Err(Self::invalid("inference receipt ordering is not canonical"));
            }
        }
        if self.rank_order.len() != self.candidate_order.len()
            || self.rank_order.iter().collect::<BTreeSet<_>>().len() != self.candidate_order.len()
            || self
                .rank_order
                .iter()
                .any(|id| !self.candidate_order.contains(id))
        {
            return Err(Self::invalid("rank order is not a candidate permutation"));
        }
        if self
            .decisions
            .iter()
            .map(|decision| decision.release_id.as_str())
            .collect::<Vec<_>>()
            != self
                .candidate_order
                .iter()
                .map(String::as_str)
                .collect::<Vec<_>>()
        {
            return Err(Self::invalid(
                "inference decisions do not match candidate order",
            ));
        }
        let classified = self
            .qualified_order
            .iter()
            .chain(self.conditional_order.iter())
            .chain(self.blocked_order.iter())
            .chain(self.unknown_order.iter())
            .cloned()
            .collect::<BTreeSet<_>>();
        if classified.len() != self.candidate_order.len()
            || classified
                .iter()
                .any(|id| !self.candidate_order.contains(id))
        {
            return Err(Self::invalid(
                "inference dispositions do not partition candidates",
            ));
        }
        if self
            .qualified_origin_order
            .iter()
            .any(|origin| !self.origin_order.contains(origin))
        {
            return Err(Self::invalid("qualified origin is not in origin order"));
        }
        if self.effect_receipts.iter().any(|effect| {
            !effect.starts_with("infer:publication-release:")
                && effect != "local-only:publication-release"
                && effect != "block:unsafe-release"
        }) {
            return Err(Self::invalid(
                "inference effect is outside the recommendation gate",
            ));
        }
        self.artifact.validate_metadata().map_err(|error| {
            FederatedPublicationReleaseInferenceError::Artifact(error.to_string())
        })?;
        if self.artifact.artifact_id != format!("{}:publication-release-inference", self.request_id)
            || self.artifact.content_type
                != "application/vnd.aurora.publication-release-inference+json"
        {
            return Err(FederatedPublicationReleaseInferenceError::Artifact(
                "artifact identity or content type does not match the inference receipt".into(),
            ));
        }
        let payload = json!({
            "schema_version": self.schema_version,
            "feature_id": self.feature_id,
            "request_id": self.request_id,
            "federation_id": self.federation_id,
            "purpose": self.purpose,
            "semantic_profile": self.semantic_profile,
            "admission": self.admission,
            "candidate_order": self.candidate_order,
            "rank_order": self.rank_order,
            "decisions": self.decisions,
            "checkpoint_digest": self.checkpoint_digest,
            "inference_digest": self.inference_digest,
            "replay_identity": self.replay_identity,
            "semantic_loss": self.semantic_loss,
            "omissions": self.omissions,
            "uncertainty": self.uncertainty,
            "negative_evidence": self.negative_evidence,
            "boundary": self.boundary,
        });
        self.artifact
            .verify_payload(&payload)
            .map_err(|error| FederatedPublicationReleaseInferenceError::Artifact(error.to_string()))
    }

    fn invalid(message: &str) -> FederatedPublicationReleaseInferenceError {
        FederatedPublicationReleaseInferenceError::Invalid(message.into())
    }

    pub fn digest(&self) -> Result<ContentHash, FederatedPublicationReleaseInferenceError> {
        self.validate()?;
        let value = serde_json::to_value(self).map_err(|error| {
            FederatedPublicationReleaseInferenceError::Serialization(error.to_string())
        })?;
        ContentHash::of_value(&value).map_err(|error| {
            FederatedPublicationReleaseInferenceError::Serialization(error.to_string())
        })
    }
}

pub fn federated_publication_release_inference_manifest() -> CapabilityManifest {
    CapabilityManifest {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        capability_id: FEATURE_ID.into(),
        version: FEATURE_VERSION.into(),
        owner_crate: "services".into(),
        consumers: [
            "publication operator".into(),
            "federation scheduler".into(),
            "release governance board".into(),
        ]
        .into(),
        behavior: "ranks digest-only federated research-object release attestations and emits qualified, degraded, unknown, or blocked recommendations without signing or publishing".into(),
        value: "reduces continual release triage while preserving provenance, omissions, negative findings, locality, and fail-closed evidence states".into(),
        inputs: vec![TypedPort {
            name: "federated_publication_release_inference_batch".into(),
            schema: INPUT_SCHEMA.into(),
            required: true,
        }],
        outputs: vec![TypedPort {
            name: "federated_publication_release_inference_receipt".into(),
            schema: OUTPUT_SCHEMA.into(),
            required: true,
        }],
        effects: [Effect::ReadLocalData, Effect::ExecuteLocalComputation, Effect::WriteLocalArtifact]
            .into(),
        permissions: ["infer:publication-release".into(), "local-only:publication-release".into()]
            .into(),
        determinism: Determinism::ByteStable,
        evidence: vec![
            EvidenceReference {
                source_id: "ro-crate-1.3".into(),
                state: EvidenceState::Supported,
                locator: Some("https://www.researchobject.org/ro-crate/specification.html".into()),
            },
            EvidenceReference {
                source_id: "w3c-prov-o".into(),
                state: EvidenceState::Supported,
                locator: Some("https://www.w3.org/TR/prov-o/".into()),
            },
        ],
        authority_requirements: vec![AuthorityRequirement {
            role: "release governance reviewer".into(),
            reason: "inference is advisory; publication and signing remain separate authorized effects".into(),
        }],
        autonomy_tier: AutonomyTier::A1,
        surfaces: [
            ResearchSurface::Cli,
            ResearchSurface::Api,
            ResearchSurface::Sdk,
            ResearchSurface::McpTool,
            ResearchSurface::Operator,
        ]
        .into(),
        boundary: PRECLINICAL_BOUNDARY.into(),
    }
}

pub fn infer_federated_publication_release(
    request: &FederatedPublicationReleaseInferenceRequest,
) -> Result<FederatedPublicationReleaseInferenceReceipt, FederatedPublicationReleaseInferenceError>
{
    if request.request_id.trim().is_empty()
        || request.federation_id.trim().is_empty()
        || request.purpose.trim().is_empty()
        || request.semantic_profile.trim().is_empty()
        || request.required_quorum == 0
        || request.capacity == 0
        || request.active_runs > request.capacity
        || request.checkpoint_seq == 0
        || request.candidates.is_empty()
        || !request.raw_data_local
        || request.boundary != PRECLINICAL_BOUNDARY
        || !digest(&request.replay_identity)
    {
        return Err(FederatedPublicationReleaseInferenceError::Invalid(
            "request identity, purpose/profile, quorum, capacity, checkpoint, candidates, locality, replay, or boundary is invalid".into(),
        ));
    }
    let mut candidates = request.candidates.clone();
    candidates.sort_by(|left, right| left.release_id.cmp(&right.release_id));
    if candidates
        .windows(2)
        .any(|pair| pair[0].release_id == pair[1].release_id)
        || candidates.iter().any(|candidate| {
            candidate.release_id.trim().is_empty()
                || candidate.origin.trim().is_empty()
                || candidate.purpose.trim().is_empty()
                || candidate.semantic_profile.trim().is_empty()
                || !digest(&candidate.artifact_digest)
                || !digest(&candidate.evidence_digest)
                || !digest(&candidate.provenance_digest)
                || !digest(&candidate.replay_identity)
        })
    {
        return Err(FederatedPublicationReleaseInferenceError::Invalid(
            "release identities and content digests must be unique, non-empty, and valid".into(),
        ));
    }
    let origin_order = unique_sorted(
        candidates
            .iter()
            .map(|candidate| candidate.origin.clone())
            .collect(),
    );
    if origin_order.len() < request.required_quorum as usize {
        return Err(FederatedPublicationReleaseInferenceError::Invalid(
            "federated publication inference requires the declared origin quorum".into(),
        ));
    }
    let global_failed = [
        ("policy-allow", !request.policy_allow),
        ("protected-closure", !request.protected_closure),
        ("capacity", request.active_runs >= request.capacity),
        ("network-permission", !request.network_permitted),
    ]
    .into_iter()
    .filter_map(|(gate, failed)| failed.then_some(gate.to_string()))
    .collect::<BTreeSet<_>>();
    let mut semantic_loss = Vec::new();
    let mut decisions = Vec::with_capacity(candidates.len());
    let mut qualified = Vec::new();
    let mut conditional = Vec::new();
    let mut blocked = Vec::new();
    let unknown = Vec::new();
    let mut omissions = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    let mut negative = BTreeSet::new();
    let mut score_by_id = HashMap::new();
    for candidate in &candidates {
        let mut failed = global_failed.clone();
        let mut pending = BTreeSet::new();
        if candidate.purpose != request.purpose {
            failed.insert("purpose".into());
        }
        if candidate.semantic_profile != request.semantic_profile {
            failed.insert("semantic-profile".into());
        }
        if candidate.replay_identity != request.replay_identity {
            failed.insert("replay-identity".into());
        }
        if !candidate.policy_allow {
            failed.insert("candidate-policy".into());
        }
        if !candidate.protected_closure {
            failed.insert("candidate-protected-closure".into());
        }
        if !candidate.signer_valid {
            failed.insert("signer".into());
        }
        if !candidate.raw_data_local {
            failed.insert("candidate-locality".into());
        }
        if !candidate.capability_complete {
            failed.insert("capability-completeness".into());
        }
        let state_score = evidence_score(candidate.evidence_state);
        let score = state_score + i32::try_from(candidate.freshness_seq.min(20)).unwrap_or(20)
            - i32::try_from(candidate.omission_count.min(20)).unwrap_or(20) * 2;
        score_by_id.insert(candidate.release_id.clone(), score);
        match candidate.evidence_state {
            EvidenceState::Contradicted => {
                failed.insert("contradicted-evidence".into());
            }
            EvidenceState::Unknown | EvidenceState::Speculative => {
                pending.insert("evidence-state".into());
                uncertainty.insert(format!("{}:evidence-state", candidate.release_id));
            }
            EvidenceState::Proven | EvidenceState::Supported => {}
        }
        if candidate.omission_count > 0 {
            pending.insert("omission-closure".into());
            omissions.insert(format!(
                "{}:omissions={}",
                candidate.release_id, candidate.omission_count
            ));
        }
        if candidate.negative_result {
            negative.insert(format!("{}:negative-result", candidate.release_id));
        } else {
            negative.insert(format!(
                "{}:negative-result-not-observed",
                candidate.release_id
            ));
        }
        if !failed.is_empty() {
            blocked.push(candidate.release_id.clone());
        } else if !pending.is_empty() {
            conditional.push(candidate.release_id.clone());
        } else {
            qualified.push(candidate.release_id.clone());
        }
        decisions.push(PublicationReleaseInferenceDecision {
            release_id: candidate.release_id.clone(),
            origin: candidate.origin.clone(),
            score,
            disposition: if !failed.is_empty() {
                "blocked"
            } else if !pending.is_empty() {
                "conditional"
            } else {
                "qualified"
            }
            .into(),
            failed_gates: failed.into_iter().collect(),
            conditional_gates: pending.into_iter().collect(),
            negative_result: candidate.negative_result,
        });
        if decisions
            .last()
            .is_some_and(|decision| !decision.failed_gates.is_empty())
        {
            semantic_loss.push(SemanticLoss {
                field: format!("release:{}", candidate.release_id),
                reason: "release attestation failed one or more publication inference gates".into(),
                severity: LossSeverity::DecisionRelevant,
            });
        }
    }
    let candidate_order = candidates
        .iter()
        .map(|candidate| candidate.release_id.clone())
        .collect::<Vec<_>>();
    let mut rank_order = candidate_order.clone();
    rank_order.sort_by(|left, right| {
        score_by_id[right]
            .cmp(&score_by_id[left])
            .then_with(|| left.cmp(right))
    });
    let qualified_origin_order = unique_sorted(
        candidates
            .iter()
            .filter(|candidate| qualified.contains(&candidate.release_id))
            .map(|candidate| candidate.origin.clone())
            .collect(),
    );
    let admission = if !global_failed.is_empty() || !blocked.is_empty() {
        "blocked"
    } else if !conditional.is_empty() || !request.network_permitted {
        "degraded"
    } else if qualified.is_empty() {
        "unknown"
    } else {
        "qualified"
    };
    let checkpoint_digest = ContentHash::of_value(&json!({
        "federation_id": request.federation_id,
        "checkpoint_seq": request.checkpoint_seq,
        "candidate_order": candidate_order,
        "origin_order": origin_order,
    }))
    .map_err(|error| FederatedPublicationReleaseInferenceError::Serialization(error.to_string()))?;
    let inference_digest = ContentHash::of_value(&json!({
        "admission": admission,
        "rank_order": rank_order,
        "decisions": decisions,
        "semantic_loss": semantic_loss,
    }))
    .map_err(|error| FederatedPublicationReleaseInferenceError::Serialization(error.to_string()))?;
    let effect_receipts = if admission == "qualified" {
        vec![format!(
            "infer:publication-release:{}",
            request.federation_id
        )]
    } else if admission == "degraded" && !request.network_permitted {
        vec![
            "infer:publication-release:local".into(),
            "local-only:publication-release".into(),
        ]
    } else {
        vec!["block:unsafe-release".into()]
    };
    let payload = json!({
        "schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION,
        "feature_id": FEATURE_ID,
        "request_id": request.request_id,
        "federation_id": request.federation_id,
        "purpose": request.purpose,
        "semantic_profile": request.semantic_profile,
        "admission": admission,
        "candidate_order": candidate_order,
        "rank_order": rank_order,
        "decisions": decisions,
        "checkpoint_digest": checkpoint_digest,
        "inference_digest": inference_digest,
        "replay_identity": request.replay_identity,
        "semantic_loss": semantic_loss,
        "omissions": omissions,
        "uncertainty": uncertainty,
        "negative_evidence": negative,
        "boundary": PRECLINICAL_BOUNDARY,
    });
    let artifact = TypedResearchArtifact::from_payload(
        format!("{}:publication-release-inference", request.request_id),
        "application/vnd.aurora.publication-release-inference+json",
        &payload,
        semantic_loss.clone(),
        vec![ProvenanceLink {
            source_id: request.federation_id.clone(),
            relation: "inference-over-release-attestations".into(),
            digest: inference_digest.clone(),
        }],
    )
    .map_err(|error| FederatedPublicationReleaseInferenceError::Artifact(error.to_string()))?;
    let receipt = FederatedPublicationReleaseInferenceReceipt {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        contract_version: FEATURE_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        request_id: request.request_id.clone(),
        federation_id: request.federation_id.clone(),
        purpose: request.purpose.clone(),
        semantic_profile: request.semantic_profile.clone(),
        admission: admission.into(),
        origin_order,
        qualified_origin_order,
        candidate_order,
        rank_order,
        qualified_order: unique_sorted(qualified),
        conditional_order: unique_sorted(conditional),
        blocked_order: unique_sorted(blocked),
        unknown_order: unknown,
        decisions,
        checkpoint_seq: request.checkpoint_seq,
        checkpoint_digest,
        inference_digest,
        replay_identity: request.replay_identity.clone(),
        semantic_loss,
        omissions: omissions.into_iter().collect(),
        uncertainty: uncertainty.into_iter().collect(),
        negative_evidence: negative.into_iter().collect(),
        effect_receipts: unique_sorted(effect_receipts),
        artifact,
        raw_data_local: true,
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    receipt.validate()?;
    Ok(receipt)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hash(byte: u8) -> ContentHash {
        ContentHash::of_bytes(&[byte; 8])
    }

    fn candidate(
        id: &str,
        origin: &str,
        state: EvidenceState,
    ) -> PublicationReleaseInferenceCandidate {
        PublicationReleaseInferenceCandidate {
            release_id: id.into(),
            origin: origin.into(),
            purpose: "replication".into(),
            semantic_profile: "preclinical-v1".into(),
            artifact_digest: hash(1),
            evidence_digest: hash(2),
            provenance_digest: hash(3),
            replay_identity: hash(9),
            evidence_state: state,
            freshness_seq: 3,
            omission_count: 0,
            negative_result: true,
            policy_allow: true,
            protected_closure: true,
            signer_valid: true,
            raw_data_local: true,
            capability_complete: true,
        }
    }

    fn request(
        candidates: Vec<PublicationReleaseInferenceCandidate>,
    ) -> FederatedPublicationReleaseInferenceRequest {
        FederatedPublicationReleaseInferenceRequest {
            request_id: "inference-1".into(),
            federation_id: "fed-1".into(),
            purpose: "replication".into(),
            semantic_profile: "preclinical-v1".into(),
            required_quorum: 2,
            capacity: 8,
            active_runs: 1,
            checkpoint_seq: 4,
            replay_identity: hash(9),
            candidates,
            policy_allow: true,
            protected_closure: true,
            raw_data_local: true,
            network_permitted: true,
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }

    #[test]
    fn manifest_is_a1_and_has_separate_publication_authority() {
        let manifest = federated_publication_release_inference_manifest();
        assert_eq!(manifest.autonomy_tier, AutonomyTier::A1);
        assert!(manifest.validate().is_ok());
        assert!(manifest.authority_requirements[0]
            .reason
            .contains("separate"));
    }

    #[test]
    fn ranking_is_deterministic_and_quorum_origins_are_retained() {
        let receipt = infer_federated_publication_release(&request(vec![
            candidate("r2", "site-b", EvidenceState::Supported),
            candidate("r1", "site-a", EvidenceState::Proven),
        ]))
        .unwrap();
        assert_eq!(receipt.candidate_order, vec!["r1", "r2"]);
        assert_eq!(receipt.rank_order, vec!["r1", "r2"]);
        assert_eq!(receipt.qualified_origin_order, vec!["site-a", "site-b"]);
        assert_eq!(receipt.admission, "qualified");
    }

    #[test]
    fn unknown_and_omitted_evidence_degrade_without_becoming_a_pass() {
        let mut unknown_candidate = candidate("r1", "site-a", EvidenceState::Unknown);
        unknown_candidate.omission_count = 2;
        let receipt = infer_federated_publication_release(&request(vec![
            unknown_candidate,
            candidate("r2", "site-b", EvidenceState::Supported),
        ]))
        .unwrap();
        assert_eq!(receipt.admission, "degraded");
        assert_eq!(receipt.conditional_order, vec!["r1"]);
        assert!(receipt
            .omissions
            .iter()
            .any(|omission| omission.contains("r1")));
        assert!(receipt
            .effect_receipts
            .contains(&"block:unsafe-release".into()));
    }

    #[test]
    fn contradicted_or_policy_denied_release_is_blocked() {
        let mut denied = candidate("r1", "site-a", EvidenceState::Contradicted);
        denied.policy_allow = false;
        let receipt = infer_federated_publication_release(&request(vec![
            denied,
            candidate("r2", "site-b", EvidenceState::Supported),
        ]))
        .unwrap();
        assert_eq!(receipt.admission, "blocked");
        assert!(receipt.blocked_order.contains(&"r1".into()));
        assert_eq!(receipt.effect_receipts, vec!["block:unsafe-release"]);
    }

    #[test]
    fn locality_degrades_to_local_only_without_exporting_raw_data() {
        let mut request = request(vec![
            candidate("r1", "site-a", EvidenceState::Supported),
            candidate("r2", "site-b", EvidenceState::Supported),
        ]);
        request.network_permitted = false;
        let receipt = infer_federated_publication_release(&request).unwrap();
        assert_eq!(receipt.admission, "blocked");
        assert!(receipt.raw_data_local);
        assert!(receipt
            .effect_receipts
            .contains(&"block:unsafe-release".into()));
    }

    #[test]
    fn tampered_artifact_payload_identity_is_rejected() {
        let mut receipt = infer_federated_publication_release(&request(vec![
            candidate("r1", "site-a", EvidenceState::Supported),
            candidate("r2", "site-b", EvidenceState::Supported),
        ]))
        .unwrap();
        receipt.artifact.content_hash = ContentHash::of_bytes(b"tampered");
        assert!(matches!(
            receipt.validate(),
            Err(FederatedPublicationReleaseInferenceError::Artifact(_))
        ));
    }
}
