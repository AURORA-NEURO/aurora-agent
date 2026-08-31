//! Federated continual knowledge-representation assurance for `AFA-registry-P04-F28`.
//!
//! This registry-owned verifier accepts only typed, purpose-bound claim attestations and signed
//! aggregate peer summaries. It emits a deterministic `TypedKnowledgeWorld7` release receipt;
//! raw study payloads stay at their origin and no biological or clinical conclusion is inferred.

use bioprism_foundation::{
    AutonomyTier, CapabilityManifest, Determinism, Effect, EvidenceReference, EvidenceState,
    ResearchSurface, TypedPort, TypedResearchArtifact, PRECLINICAL_BOUNDARY,
    RESEARCH_CONTRACT_SCHEMA_VERSION,
};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-registry-P04-F28";
pub const CONTRACT_VERSION: &str =
    "registry-federated-continual-knowledge-representation-assurance-harness/1.0";
pub const INPUT_SCHEMA: &str = "ScopedResearchClaims4@1";
pub const OUTPUT_SCHEMA: &str = "TypedKnowledgeWorld7@1";
pub const TOOL_NAME: &str = "registry_knowledge_representation_assurance";
const CONTENT_TYPE: &str = "application/vnd.aurora.registry-typed-knowledge-world-7+json";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScopedClaim {
    pub claim_id: String,
    pub scope: String,
    pub purpose: String,
    pub semantic_profile: String,
    pub proposition_digest: ContentHash,
    pub evidence_digest: ContentHash,
    pub provenance_digest: ContentHash,
    pub replay_identity: ContentHash,
    pub state: EvidenceState,
    pub local_only: bool,
    pub permitted: bool,
    pub negative_result: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KnowledgePeer {
    pub peer_id: String,
    pub purpose: String,
    pub semantic_profile: String,
    pub summary_digest: ContentHash,
    pub claim_order: Vec<String>,
    pub signed: bool,
    pub aggregate_only: bool,
    pub raw_data_local: bool,
    pub state: EvidenceState,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScopedResearchClaims {
    pub schema_version: String,
    pub request_id: String,
    pub registry_id: String,
    pub purpose: String,
    pub semantic_profile: String,
    pub required_claim_order: Vec<String>,
    pub claims: Vec<ScopedClaim>,
    pub peers: Vec<KnowledgePeer>,
    pub minimum_peer_quorum: u16,
    pub replay_identity: ContentHash,
    pub policy_allow: bool,
    pub protected_closure: bool,
    pub signed_approval: bool,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub adversarial_events: Vec<String>,
    pub boundary: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum KnowledgeDisposition {
    Qualified,
    Unresolved,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TypedKnowledgeWorld {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub registry_id: String,
    pub purpose: String,
    pub semantic_profile: String,
    pub disposition: KnowledgeDisposition,
    pub claim_order: Vec<String>,
    pub selected_claim_order: Vec<String>,
    pub unresolved_claim_order: Vec<String>,
    pub blocked_claim_order: Vec<String>,
    pub missing_claim_order: Vec<String>,
    pub peer_order: Vec<String>,
    pub qualified_peer_order: Vec<String>,
    pub missing_peer_order: Vec<String>,
    pub omission_order: Vec<String>,
    pub uncertainty_order: Vec<String>,
    pub negative_evidence_order: Vec<String>,
    pub replay_identity: ContentHash,
    pub world_digest: ContentHash,
    pub artifact: TypedResearchArtifact,
    pub effect_receipts: Vec<String>,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub boundary: String,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum KnowledgeAssuranceError {
    #[error("invalid scoped research claims: {0}")]
    Invalid(String),
    #[error("typed knowledge world artifact failed: {0}")]
    Artifact(String),
}

fn invalid(message: impl Into<String>) -> KnowledgeAssuranceError {
    KnowledgeAssuranceError::Invalid(message.into())
}
fn digest_is_valid(value: &ContentHash) -> bool {
    value.as_str().len() == 64
}
fn canonical(values: &[String]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

impl TypedKnowledgeWorld {
    pub fn validate(&self) -> Result<(), KnowledgeAssuranceError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.boundary != PRECLINICAL_BOUNDARY
            || !self.raw_data_local
            || !self.aggregate_only
            || self.request_id.trim().is_empty()
            || self.registry_id.trim().is_empty()
            || self.purpose.trim().is_empty()
            || self.semantic_profile.trim().is_empty()
            || self.claim_order.is_empty()
            || self.peer_order.is_empty()
            || self.effect_receipts.is_empty()
        {
            return Err(invalid(
                "knowledge identity, claims, peers, locality, or effects are incomplete",
            ));
        }
        for values in [
            &self.claim_order,
            &self.selected_claim_order,
            &self.unresolved_claim_order,
            &self.blocked_claim_order,
            &self.missing_claim_order,
            &self.peer_order,
            &self.qualified_peer_order,
            &self.missing_peer_order,
            &self.omission_order,
            &self.uncertainty_order,
            &self.negative_evidence_order,
            &self.effect_receipts,
        ] {
            if !canonical(values) {
                return Err(invalid("knowledge ordering is not canonical"));
            }
        }
        let claims = self.claim_order.iter().cloned().collect::<BTreeSet<_>>();
        let parts = self
            .selected_claim_order
            .iter()
            .chain(self.unresolved_claim_order.iter())
            .chain(self.blocked_claim_order.iter())
            .cloned()
            .collect::<Vec<_>>();
        if parts.len() != claims.len() || parts.iter().cloned().collect::<BTreeSet<_>>() != claims {
            return Err(invalid("knowledge claims do not partition"));
        }
        let peers = self.peer_order.iter().cloned().collect::<BTreeSet<_>>();
        let peer_parts = self
            .qualified_peer_order
            .iter()
            .chain(self.missing_peer_order.iter())
            .cloned()
            .collect::<Vec<_>>();
        if peer_parts.len() != peers.len()
            || peer_parts.iter().cloned().collect::<BTreeSet<_>>() != peers
        {
            return Err(invalid("knowledge peers do not partition"));
        }
        if ![
            &self.replay_identity,
            &self.world_digest,
            &self.artifact.content_hash,
        ]
        .iter()
        .all(|value| digest_is_valid(value))
        {
            return Err(invalid("knowledge digest is invalid"));
        }
        self.artifact
            .validate_metadata()
            .map_err(|error| KnowledgeAssuranceError::Artifact(error.to_string()))?;
        if self.artifact.content_type != CONTENT_TYPE {
            return Err(invalid("knowledge artifact type is invalid"));
        }
        let expected = if self.disposition == KnowledgeDisposition::Qualified {
            vec![format!("verify:typed-knowledge-world:{}", self.request_id)]
        } else {
            vec!["block:unsafe-release".into()]
        };
        if self.effect_receipts != expected {
            return Err(invalid("knowledge effect receipt is invalid"));
        }
        Ok(())
    }
}

pub fn knowledge_representation_assurance_manifest() -> CapabilityManifest {
    CapabilityManifest {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        capability_id: FEATURE_ID.into(),
        version: CONTRACT_VERSION.into(),
        owner_crate: "registry".into(),
        consumers: BTreeSet::from([
            String::from("registry steward"),
            String::from("benchmark curator"),
            String::from("research workflow operator"),
        ]),
        behavior: "verifies scoped claims and signed aggregate peer summaries into a deterministic typed knowledge-world receipt without moving raw studies".into(),
        value: "makes federated knowledge closure and contradiction visible before a research object is released".into(),
        inputs: vec![TypedPort { name: "scoped_research_claims".into(), schema: INPUT_SCHEMA.into(), required: true }],
        outputs: vec![TypedPort { name: "typed_knowledge_world".into(), schema: OUTPUT_SCHEMA.into(), required: true }],
        effects: BTreeSet::from([Effect::ReadLocalData]),
        permissions: BTreeSet::from([String::from("evaluate:capability-runs")]),
        determinism: Determinism::ByteStable,
        evidence: vec![EvidenceReference { source_id: "w3c-prov-o".into(), state: EvidenceState::Supported, locator: Some("https://www.w3.org/TR/prov-o/".into()) }],
        authority_requirements: Vec::new(),
        autonomy_tier: AutonomyTier::A1,
        surfaces: BTreeSet::from([ResearchSurface::Ui, ResearchSurface::Api, ResearchSurface::Sdk, ResearchSurface::McpTool, ResearchSurface::Protocol, ResearchSurface::Policy, ResearchSurface::Operator]),
        boundary: PRECLINICAL_BOUNDARY.into(),
    }
}

pub fn assure_knowledge_representation(
    request: &ScopedResearchClaims,
) -> Result<TypedKnowledgeWorld, KnowledgeAssuranceError> {
    validate_request(request)?;
    let mut claims = request.claims.clone();
    claims.sort_by(|a, b| a.claim_id.cmp(&b.claim_id));
    let claim_order = claims
        .iter()
        .map(|claim| claim.claim_id.clone())
        .collect::<Vec<_>>();
    let required = request
        .required_claim_order
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let present = claim_order.iter().cloned().collect::<BTreeSet<_>>();
    let mut missing = required
        .difference(&present)
        .cloned()
        .collect::<BTreeSet<_>>();
    let mut selected = BTreeSet::new();
    let mut unresolved = BTreeSet::new();
    let mut blocked = BTreeSet::new();
    let mut omissions = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    let mut negative = BTreeSet::new();
    for claim in &claims {
        if claim.negative_result {
            negative.insert(format!("{}:negative-result", claim.claim_id));
        }
        if !required.contains(&claim.claim_id) {
            omissions.insert(format!("{}:not-required", claim.claim_id));
        }
        if claim.semantic_profile != request.semantic_profile {
            uncertainty.insert(format!("{}:semantic-profile-mismatch", claim.claim_id));
            unresolved.insert(claim.claim_id.clone());
        } else if claim.state == EvidenceState::Contradicted
            || !claim.local_only
            || !claim.permitted
        {
            blocked.insert(claim.claim_id.clone());
        } else if matches!(
            claim.state,
            EvidenceState::Unknown | EvidenceState::Speculative
        ) {
            unresolved.insert(claim.claim_id.clone());
        } else if required.contains(&claim.claim_id) {
            selected.insert(claim.claim_id.clone());
        } else {
            unresolved.insert(claim.claim_id.clone());
        }
    }
    let mut peer_order = BTreeSet::new();
    let mut qualified_peers = BTreeSet::new();
    let mut missing_peers = BTreeSet::new();
    let mut contradictory_peer = false;
    for peer in &request.peers {
        peer_order.insert(peer.peer_id.clone());
        let comparable = peer.purpose == request.purpose
            && peer.semantic_profile == request.semantic_profile
            && peer.signed
            && peer.aggregate_only
            && peer.raw_data_local
            && digest_is_valid(&peer.summary_digest)
            && !peer.claim_order.is_empty()
            && canonical(&peer.claim_order);
        if peer.state == EvidenceState::Contradicted {
            contradictory_peer = true;
        }
        if comparable && matches!(peer.state, EvidenceState::Proven | EvidenceState::Supported) {
            qualified_peers.insert(peer.peer_id.clone());
        } else {
            missing_peers.insert(peer.peer_id.clone());
            uncertainty.insert(format!("peer:{}:not-qualified", peer.peer_id));
        }
    }
    if qualified_peers.len() < request.minimum_peer_quorum as usize {
        uncertainty.insert("peer:minimum-quorum-unmet".into());
    }
    if !request.policy_allow {
        negative.insert("request:policy-denied".into());
    }
    if !request.protected_closure {
        uncertainty.insert("request:protected-closure-incomplete".into());
    }
    if !request.signed_approval {
        uncertainty.insert("request:signed-approval-missing".into());
    }
    negative.extend(
        request
            .adversarial_events
            .iter()
            .map(|event| format!("adversarial:{event}")),
    );
    let global_block = !request.policy_allow
        || !request.protected_closure
        || !request.signed_approval
        || !request.raw_data_local
        || !request.aggregate_only
        || !request.adversarial_events.is_empty()
        || contradictory_peer;
    if global_block {
        blocked.extend(claim_order.iter().cloned());
        selected.clear();
        unresolved.clear();
        missing.clear();
        omissions.insert("request:knowledge-gate-blocked".into());
    }
    let disposition = if global_block || !blocked.is_empty() {
        KnowledgeDisposition::Blocked
    } else if !missing.is_empty()
        || !unresolved.is_empty()
        || qualified_peers.len() < request.minimum_peer_quorum as usize
    {
        KnowledgeDisposition::Unresolved
    } else {
        KnowledgeDisposition::Qualified
    };
    let selected_claim_order = selected.into_iter().collect::<Vec<_>>();
    let unresolved_claim_order = unresolved.into_iter().collect::<Vec<_>>();
    let blocked_claim_order = blocked.into_iter().collect::<Vec<_>>();
    let missing_claim_order = missing.into_iter().collect::<Vec<_>>();
    let peer_order = peer_order.into_iter().collect::<Vec<_>>();
    let qualified_peer_order = qualified_peers.into_iter().collect::<Vec<_>>();
    let missing_peer_order = missing_peers.into_iter().collect::<Vec<_>>();
    let omission_order = omissions.into_iter().collect::<Vec<_>>();
    let uncertainty_order = uncertainty.into_iter().collect::<Vec<_>>();
    let negative_evidence_order = negative.into_iter().collect::<Vec<_>>();
    let effect_receipts = if disposition == KnowledgeDisposition::Qualified {
        vec![format!(
            "verify:typed-knowledge-world:{}",
            request.request_id
        )]
    } else {
        vec!["block:unsafe-release".into()]
    };
    let evidence = json!({"claim_order":claim_order,"selected_claim_order":selected_claim_order,"unresolved_claim_order":unresolved_claim_order,"blocked_claim_order":blocked_claim_order,"missing_claim_order":missing_claim_order,"peer_order":peer_order,"qualified_peer_order":qualified_peer_order,"missing_peer_order":missing_peer_order,"omission_order":omission_order,"uncertainty_order":uncertainty_order,"negative_evidence_order":negative_evidence_order});
    let world_payload = json!({"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"contract_version":CONTRACT_VERSION,"feature_id":FEATURE_ID,"request_id":request.request_id,"registry_id":request.registry_id,"purpose":request.purpose,"semantic_profile":request.semantic_profile,"disposition":disposition,"evidence":evidence,"replay_identity":request.replay_identity,"effect_receipts":effect_receipts,"raw_data_local":request.raw_data_local,"aggregate_only":request.aggregate_only,"boundary":PRECLINICAL_BOUNDARY});
    let world_digest = ContentHash::of_value(&world_payload)
        .map_err(|error| KnowledgeAssuranceError::Artifact(error.to_string()))?;
    let artifact = TypedResearchArtifact::from_payload(
        format!("registry-typed-knowledge-world:{}", request.request_id),
        CONTENT_TYPE,
        &world_payload,
        Vec::new(),
        Vec::new(),
    )
    .map_err(|error| KnowledgeAssuranceError::Artifact(error.to_string()))?;
    let strings = |value: &Value| {
        value
            .as_array()
            .unwrap()
            .iter()
            .map(|item| item.as_str().unwrap().to_string())
            .collect::<Vec<_>>()
    };
    let section = TypedKnowledgeWorld {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        request_id: request.request_id.clone(),
        registry_id: request.registry_id.clone(),
        purpose: request.purpose.clone(),
        semantic_profile: request.semantic_profile.clone(),
        disposition,
        claim_order: strings(evidence.get("claim_order").unwrap()),
        selected_claim_order: strings(evidence.get("selected_claim_order").unwrap()),
        unresolved_claim_order: strings(evidence.get("unresolved_claim_order").unwrap()),
        blocked_claim_order: strings(evidence.get("blocked_claim_order").unwrap()),
        missing_claim_order: strings(evidence.get("missing_claim_order").unwrap()),
        peer_order: strings(evidence.get("peer_order").unwrap()),
        qualified_peer_order: strings(evidence.get("qualified_peer_order").unwrap()),
        missing_peer_order: strings(evidence.get("missing_peer_order").unwrap()),
        omission_order: strings(evidence.get("omission_order").unwrap()),
        uncertainty_order: strings(evidence.get("uncertainty_order").unwrap()),
        negative_evidence_order: strings(evidence.get("negative_evidence_order").unwrap()),
        replay_identity: request.replay_identity.clone(),
        world_digest,
        artifact,
        effect_receipts: strings(world_payload.get("effect_receipts").unwrap()),
        raw_data_local: request.raw_data_local,
        aggregate_only: request.aggregate_only,
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    section.validate()?;
    Ok(section)
}

pub fn assure_knowledge_representation_json(value: &Value) -> Result<Value, String> {
    let request: ScopedResearchClaims = serde_json::from_value(value.clone())
        .map_err(|error| format!("invalid scoped research claims: {error}"))?;
    let world = assure_knowledge_representation(&request).map_err(|error| error.to_string())?;
    serde_json::to_value(world)
        .map_err(|error| format!("cannot serialize typed knowledge world: {error}"))
}

pub fn validate_knowledge_representation_json(
    value: &Value,
) -> Result<TypedKnowledgeWorld, String> {
    let world: TypedKnowledgeWorld = serde_json::from_value(value.clone())
        .map_err(|error| format!("invalid typed knowledge world: {error}"))?;
    world.validate().map_err(|error| error.to_string())?;
    Ok(world)
}

fn validate_request(request: &ScopedResearchClaims) -> Result<(), KnowledgeAssuranceError> {
    if request.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
        || request.request_id.trim().is_empty()
        || request.registry_id.trim().is_empty()
        || request.purpose.trim().is_empty()
        || request.semantic_profile.trim().is_empty()
        || request.required_claim_order.is_empty()
        || !canonical(&request.required_claim_order)
        || request.claims.is_empty()
        || request.peers.is_empty()
        || request.minimum_peer_quorum == 0
        || !digest_is_valid(&request.replay_identity)
        || !canonical(&request.adversarial_events)
        || !request.raw_data_local
        || !request.aggregate_only
        || request.boundary != PRECLINICAL_BOUNDARY
    {
        return Err(invalid(
            "claims identity, closure, quorum, replay, locality, or boundary is invalid",
        ));
    }
    let mut claim_ids = BTreeSet::new();
    for claim in &request.claims {
        if claim.claim_id.trim().is_empty()
            || !claim_ids.insert(claim.claim_id.clone())
            || claim.scope.trim().is_empty()
            || claim.purpose != request.purpose
            || claim.semantic_profile.trim().is_empty()
            || claim.proposition_digest.as_str().len() != 64
            || claim.evidence_digest.as_str().len() != 64
            || claim.provenance_digest.as_str().len() != 64
            || claim.replay_identity.as_str().len() != 64
        {
            return Err(invalid(format!(
                "claim {} is malformed or duplicated",
                claim.claim_id
            )));
        }
    }
    let mut peer_ids = BTreeSet::new();
    for peer in &request.peers {
        if peer.peer_id.trim().is_empty()
            || !peer_ids.insert(peer.peer_id.clone())
            || peer.purpose.trim().is_empty()
            || peer.semantic_profile.trim().is_empty()
            || peer.claim_order.is_empty()
            || !canonical(&peer.claim_order)
            || !digest_is_valid(&peer.summary_digest)
        {
            return Err(invalid(format!(
                "peer {} is malformed or duplicated",
                peer.peer_id
            )));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn hash(value: &str) -> ContentHash {
        ContentHash::of_bytes(value.as_bytes())
    }
    fn request() -> ScopedResearchClaims {
        let digest = hash("knowledge");
        let claim = |id: &str| ScopedClaim {
            claim_id: id.into(),
            scope: "study:local".into(),
            purpose: "mechanism-screen".into(),
            semantic_profile: "preclinical:v1".into(),
            proposition_digest: digest.clone(),
            evidence_digest: digest.clone(),
            provenance_digest: digest.clone(),
            replay_identity: digest.clone(),
            state: EvidenceState::Supported,
            local_only: true,
            permitted: true,
            negative_result: false,
        };
        let peer = KnowledgePeer {
            peer_id: "peer:a".into(),
            purpose: "mechanism-screen".into(),
            semantic_profile: "preclinical:v1".into(),
            summary_digest: digest.clone(),
            claim_order: vec!["claim:a".into(), "claim:b".into()],
            signed: true,
            aggregate_only: true,
            raw_data_local: true,
            state: EvidenceState::Supported,
        };
        ScopedResearchClaims {
            schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
            request_id: "knowledge:one".into(),
            registry_id: "registry:test".into(),
            purpose: "mechanism-screen".into(),
            semantic_profile: "preclinical:v1".into(),
            required_claim_order: vec!["claim:a".into(), "claim:b".into()],
            claims: vec![claim("claim:b"), claim("claim:a")],
            peers: vec![peer],
            minimum_peer_quorum: 1,
            replay_identity: digest,
            policy_allow: true,
            protected_closure: true,
            signed_approval: true,
            raw_data_local: true,
            aggregate_only: true,
            adversarial_events: vec![],
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }
    #[test]
    fn manifest_is_a1() {
        assert_eq!(
            knowledge_representation_assurance_manifest().autonomy_tier,
            AutonomyTier::A1
        );
    }
    #[test]
    fn qualified_is_deterministic() {
        let a = assure_knowledge_representation(&request()).unwrap();
        let b = assure_knowledge_representation(&request()).unwrap();
        assert_eq!(a.disposition, KnowledgeDisposition::Qualified);
        assert_eq!(a.world_digest, b.world_digest);
    }
    #[test]
    fn missing_claim_is_unresolved() {
        let mut value = request();
        value.claims.pop();
        assert_eq!(
            assure_knowledge_representation(&value).unwrap().disposition,
            KnowledgeDisposition::Unresolved
        );
    }
    #[test]
    fn unknown_claim_is_unresolved() {
        let mut value = request();
        value.claims[0].state = EvidenceState::Unknown;
        assert_eq!(
            assure_knowledge_representation(&value).unwrap().disposition,
            KnowledgeDisposition::Unresolved
        );
    }
    #[test]
    fn contradiction_blocks() {
        let mut value = request();
        value.claims[0].state = EvidenceState::Contradicted;
        assert_eq!(
            assure_knowledge_representation(&value).unwrap().disposition,
            KnowledgeDisposition::Blocked
        );
    }
    #[test]
    fn policy_denial_blocks() {
        let mut value = request();
        value.policy_allow = false;
        assert_eq!(
            assure_knowledge_representation(&value).unwrap().disposition,
            KnowledgeDisposition::Blocked
        );
    }
    #[test]
    fn quorum_gap_is_unresolved() {
        let mut value = request();
        value.minimum_peer_quorum = 2;
        assert_eq!(
            assure_knowledge_representation(&value).unwrap().disposition,
            KnowledgeDisposition::Unresolved
        );
    }
}
