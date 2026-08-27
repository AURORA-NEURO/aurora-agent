//! Federated continual typed-knowledge representation contract model for `AFA-mcp-P04-F08`.
//!
//! The model admits only purpose-bound, semantically comparable claim attestations and
//! aggregate-only peer summaries into a deterministic `TypedKnowledgeWorld2` envelope. It never
//! moves raw study data, invents a biological relation, or upgrades unknown/contradicted evidence.

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

pub const FEATURE_ID: &str = "AFA-mcp-P04-F08";
pub const CONTRACT_VERSION: &str =
    "mcp-federated-continual-knowledge-representation-contract-model/1.0";
pub const INPUT_SCHEMA: &str = "ScopedResearchClaims4@1";
pub const OUTPUT_SCHEMA: &str = "TypedKnowledgeWorld2@1";
pub const TOOL_NAME: &str = "mcp_knowledge_representation_contract";
const CONTENT_TYPE: &str = "application/vnd.aurora.mcp-typed-knowledge-world-2+json";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KnowledgeClaimAttestation {
    pub claim_id: String,
    pub study_id: String,
    pub subject: String,
    pub predicate: String,
    pub object: String,
    pub semantic_profile: String,
    pub evidence_state: EvidenceState,
    pub artifact_digest: ContentHash,
    pub provenance_digest: ContentHash,
    pub replay_identity: ContentHash,
    pub omissions: Vec<String>,
    pub uncertainty: Vec<String>,
    pub negative_result: bool,
    pub local_only: bool,
    pub permitted: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PeerKnowledgeSummary {
    pub institution_id: String,
    pub knowledge_digest: ContentHash,
    pub semantic_profile: String,
    pub replay_identity: ContentHash,
    pub signed: bool,
    pub permitted: bool,
    pub aggregate_only: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KnowledgeRepresentationRequest {
    pub schema_version: String,
    pub request_id: String,
    pub federation_id: String,
    pub semantic_profile: String,
    pub required_claim_order: Vec<String>,
    pub claims: Vec<KnowledgeClaimAttestation>,
    pub peers: Vec<PeerKnowledgeSummary>,
    pub minimum_peer_quorum: u16,
    pub replay_identity: ContentHash,
    pub policy_allow: bool,
    pub protected_closure: bool,
    pub federation_approved: bool,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub boundary: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum KnowledgeWorldDisposition {
    Qualified,
    Unresolved,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TypedKnowledgeWorldReceipt {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub federation_id: String,
    pub semantic_profile: String,
    pub disposition: KnowledgeWorldDisposition,
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
pub enum KnowledgeRepresentationError {
    #[error("invalid knowledge representation request: {0}")]
    Invalid(String),
    #[error("knowledge representation artifact failed: {0}")]
    Artifact(String),
    #[error("knowledge representation JSON failed: {0}")]
    Json(String),
}

fn invalid(message: impl Into<String>) -> KnowledgeRepresentationError {
    KnowledgeRepresentationError::Invalid(message.into())
}
fn digest_is_valid(value: &ContentHash) -> bool {
    value.as_str().len() == 64
}
fn canonical(values: &[String]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

impl TypedKnowledgeWorldReceipt {
    pub fn validate(&self) -> Result<(), KnowledgeRepresentationError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.boundary != PRECLINICAL_BOUNDARY
            || !self.raw_data_local
            || !self.aggregate_only
            || self.request_id.trim().is_empty()
            || self.federation_id.trim().is_empty()
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
        let claim_parts = self
            .selected_claim_order
            .iter()
            .chain(self.unresolved_claim_order.iter())
            .chain(self.blocked_claim_order.iter())
            .cloned()
            .collect::<Vec<_>>();
        if claim_parts.len() != claims.len()
            || claim_parts.iter().cloned().collect::<BTreeSet<_>>() != claims
        {
            return Err(invalid("knowledge claim states do not partition claims"));
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
            return Err(invalid("knowledge peer states do not partition peers"));
        }
        for value in [
            &self.replay_identity,
            &self.world_digest,
            &self.artifact.content_hash,
        ] {
            if !digest_is_valid(value) {
                return Err(invalid("knowledge digest is invalid"));
            }
        }
        self.artifact
            .validate_metadata()
            .map_err(|error| KnowledgeRepresentationError::Artifact(error.to_string()))?;
        if self.artifact.content_type != CONTENT_TYPE {
            return Err(invalid("knowledge artifact type is invalid"));
        }
        if self.disposition == KnowledgeWorldDisposition::Qualified
            && self.effect_receipts != [format!("read:local-knowledge-world:{}", self.request_id)]
        {
            return Err(invalid("qualified knowledge effect is invalid"));
        }
        if self.disposition != KnowledgeWorldDisposition::Qualified
            && self.effect_receipts != ["block:unsafe-release"]
        {
            return Err(invalid("non-qualified knowledge must block release"));
        }
        Ok(())
    }
}

pub fn knowledge_representation_contract_manifest() -> CapabilityManifest {
    CapabilityManifest {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        capability_id: FEATURE_ID.into(),
        version: CONTRACT_VERSION.into(),
        owner_crate: "mcp".into(),
        consumers: BTreeSet::from([
            String::from("preclinical researcher"),
            String::from("research workflow operator"),
            String::from("federation steward"),
        ]),
        behavior: "validates purpose-bound typed research claims and aggregate-only peer summaries into a deterministic knowledge-world artifact without moving raw study data".into(),
        value: "gives federated research teams a stable, omission-aware knowledge representation that cannot silently promote unknown or contradictory evidence".into(),
        inputs: vec![TypedPort { name: "scoped_research_claims".into(), schema: INPUT_SCHEMA.into(), required: true }],
        outputs: vec![TypedPort { name: "typed_knowledge_world".into(), schema: OUTPUT_SCHEMA.into(), required: true }],
        effects: BTreeSet::from([Effect::ReadLocalData]),
        permissions: BTreeSet::from([String::from("read:local-research-artifacts")]),
        determinism: Determinism::ByteStable,
        evidence: vec![
            EvidenceReference { source_id: "ro-crate-1.3".into(), state: EvidenceState::Supported, locator: Some("https://www.researchobject.org/ro-crate/specification.html".into()) },
            EvidenceReference { source_id: "anndata-format".into(), state: EvidenceState::Supported, locator: Some("https://anndata.readthedocs.io/en/stable/fileformat-prose.html".into()) },
            EvidenceReference { source_id: "json-schema".into(), state: EvidenceState::Supported, locator: Some("https://json-schema.org/specification".into()) },
        ],
        authority_requirements: Vec::new(),
        autonomy_tier: AutonomyTier::A1,
        surfaces: BTreeSet::from([ResearchSurface::Ui, ResearchSurface::Api, ResearchSurface::Sdk, ResearchSurface::McpTool, ResearchSurface::Protocol, ResearchSurface::Policy, ResearchSurface::Operator]),
        boundary: PRECLINICAL_BOUNDARY.into(),
    }
}

pub fn model_knowledge_representation_contract(
    request: &KnowledgeRepresentationRequest,
) -> Result<TypedKnowledgeWorldReceipt, KnowledgeRepresentationError> {
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
        omissions.extend(
            claim
                .omissions
                .iter()
                .map(|value| format!("{}:{value}", claim.claim_id)),
        );
        uncertainty.extend(
            claim
                .uncertainty
                .iter()
                .map(|value| format!("{}:{value}", claim.claim_id)),
        );
        if claim.evidence_state == EvidenceState::Contradicted
            || !claim.local_only
            || !claim.permitted
        {
            blocked.insert(claim.claim_id.clone());
        } else if claim.evidence_state == EvidenceState::Unknown
            || claim.evidence_state == EvidenceState::Speculative
            || !claim.omissions.is_empty()
            || !claim.uncertainty.is_empty()
        {
            unresolved.insert(claim.claim_id.clone());
        } else {
            selected.insert(claim.claim_id.clone());
        }
    }
    let missing = required
        .difference(&claim_order.iter().cloned().collect())
        .cloned()
        .collect::<BTreeSet<_>>();
    omissions.extend(
        missing
            .iter()
            .map(|value| format!("{value}:required-claim-missing")),
    );
    let mut peers = request.peers.clone();
    peers.sort_by(|a, b| a.institution_id.cmp(&b.institution_id));
    let peer_order = peers
        .iter()
        .map(|peer| peer.institution_id.clone())
        .collect::<Vec<_>>();
    let mut qualified_peers = BTreeSet::new();
    let mut missing_peers = BTreeSet::new();
    for peer in &peers {
        if peer.signed
            && peer.permitted
            && peer.aggregate_only
            && peer.semantic_profile == request.semantic_profile
            && peer.replay_identity == request.replay_identity
            && digest_is_valid(&peer.knowledge_digest)
        {
            qualified_peers.insert(peer.institution_id.clone());
        } else {
            missing_peers.insert(peer.institution_id.clone());
        }
    }
    if qualified_peers.len() < request.minimum_peer_quorum as usize {
        uncertainty.insert("request:peer-quorum-incomplete".into());
    }
    if !request.policy_allow {
        negative.insert("request:policy-denied".into());
    }
    if !request.protected_closure {
        uncertainty.insert("request:protected-closure-incomplete".into());
    }
    if !request.federation_approved {
        uncertainty.insert("request:federation-approval-missing".into());
    }
    let global_block = !request.policy_allow
        || !request.protected_closure
        || !request.federation_approved
        || !request.raw_data_local
        || !request.aggregate_only;
    if global_block {
        blocked.extend(claim_order.iter().cloned());
        selected.clear();
        unresolved.clear();
        omissions.insert("request:knowledge-release-gate-blocked".into());
    }
    let required_block = required.iter().any(|id| blocked.contains(id));
    let disposition = if global_block || required_block {
        KnowledgeWorldDisposition::Blocked
    } else if required.is_subset(&selected)
        && missing.is_empty()
        && qualified_peers.len() >= request.minimum_peer_quorum as usize
        && unresolved.is_empty()
        && blocked.is_empty()
    {
        KnowledgeWorldDisposition::Qualified
    } else {
        KnowledgeWorldDisposition::Unresolved
    };
    let selected_order = selected.into_iter().collect::<Vec<_>>();
    let unresolved_order = unresolved.into_iter().collect::<Vec<_>>();
    let blocked_order = blocked.into_iter().collect::<Vec<_>>();
    let missing_claim_order = missing.into_iter().collect::<Vec<_>>();
    let qualified_peer_order = qualified_peers.into_iter().collect::<Vec<_>>();
    let missing_peer_order = missing_peers.into_iter().collect::<Vec<_>>();
    let omission_order = omissions.into_iter().collect::<Vec<_>>();
    let uncertainty_order = uncertainty.into_iter().collect::<Vec<_>>();
    let negative_evidence_order = negative.into_iter().collect::<Vec<_>>();
    let effect_receipts = if disposition == KnowledgeWorldDisposition::Qualified {
        vec![format!("read:local-knowledge-world:{}", request.request_id)]
    } else {
        vec!["block:unsafe-release".into()]
    };
    let payload = json!({"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "contract_version": CONTRACT_VERSION, "feature_id": FEATURE_ID, "request_id": request.request_id, "federation_id": request.federation_id, "semantic_profile": request.semantic_profile, "disposition": disposition, "claim_order": claim_order, "selected_claim_order": selected_order, "unresolved_claim_order": unresolved_order, "blocked_claim_order": blocked_order, "missing_claim_order": missing_claim_order, "peer_order": peer_order, "qualified_peer_order": qualified_peer_order, "missing_peer_order": missing_peer_order, "omission_order": omission_order, "uncertainty_order": uncertainty_order, "negative_evidence_order": negative_evidence_order, "replay_identity": request.replay_identity, "effect_receipts": effect_receipts, "raw_data_local": request.raw_data_local, "aggregate_only": request.aggregate_only, "boundary": PRECLINICAL_BOUNDARY});
    let world_digest = ContentHash::of_value(&payload)
        .map_err(|error| KnowledgeRepresentationError::Artifact(error.to_string()))?;
    let artifact = TypedResearchArtifact::from_payload(
        format!("mcp-typed-knowledge-world:{}", request.request_id),
        CONTENT_TYPE,
        &payload,
        Vec::new(),
        Vec::new(),
    )
    .map_err(|error| KnowledgeRepresentationError::Artifact(error.to_string()))?;
    let strings = |key: &str| {
        payload[key]
            .as_array()
            .unwrap()
            .iter()
            .map(|value| value.as_str().unwrap().into())
            .collect::<Vec<String>>()
    };
    let receipt = TypedKnowledgeWorldReceipt {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        request_id: request.request_id.clone(),
        federation_id: request.federation_id.clone(),
        semantic_profile: request.semantic_profile.clone(),
        disposition,
        claim_order: strings("claim_order"),
        selected_claim_order: strings("selected_claim_order"),
        unresolved_claim_order: strings("unresolved_claim_order"),
        blocked_claim_order: strings("blocked_claim_order"),
        missing_claim_order: strings("missing_claim_order"),
        peer_order: strings("peer_order"),
        qualified_peer_order: strings("qualified_peer_order"),
        missing_peer_order: strings("missing_peer_order"),
        omission_order: strings("omission_order"),
        uncertainty_order: strings("uncertainty_order"),
        negative_evidence_order: strings("negative_evidence_order"),
        replay_identity: request.replay_identity.clone(),
        world_digest,
        artifact,
        effect_receipts: strings("effect_receipts"),
        raw_data_local: request.raw_data_local,
        aggregate_only: request.aggregate_only,
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    receipt.validate()?;
    Ok(receipt)
}

pub fn model_knowledge_representation_contract_json(value: &Value) -> Result<Value, String> {
    let request: KnowledgeRepresentationRequest = serde_json::from_value(value.clone())
        .map_err(|error| format!("invalid knowledge representation request: {error}"))?;
    let receipt =
        model_knowledge_representation_contract(&request).map_err(|error| error.to_string())?;
    serde_json::to_value(receipt)
        .map_err(|error| format!("cannot serialize knowledge representation receipt: {error}"))
}

pub fn validate_knowledge_representation_contract_json(
    value: &Value,
) -> Result<TypedKnowledgeWorldReceipt, String> {
    let receipt: TypedKnowledgeWorldReceipt = serde_json::from_value(value.clone())
        .map_err(|error| format!("invalid knowledge representation receipt: {error}"))?;
    receipt.validate().map_err(|error| error.to_string())?;
    Ok(receipt)
}

fn validate_request(
    request: &KnowledgeRepresentationRequest,
) -> Result<(), KnowledgeRepresentationError> {
    if request.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
        || request.request_id.trim().is_empty()
        || request.federation_id.trim().is_empty()
        || request.semantic_profile.trim().is_empty()
        || request.required_claim_order.is_empty()
        || !canonical(&request.required_claim_order)
        || request.claims.is_empty()
        || request.peers.is_empty()
        || request.minimum_peer_quorum == 0
        || request.minimum_peer_quorum as usize > request.peers.len()
        || !digest_is_valid(&request.replay_identity)
        || !request.raw_data_local
        || !request.aggregate_only
        || request.boundary != PRECLINICAL_BOUNDARY
    {
        return Err(invalid(
            "knowledge identity, claim closure, quorum, replay, locality, or boundary is invalid",
        ));
    }
    let mut claims = BTreeSet::new();
    for claim in &request.claims {
        if claim.claim_id.trim().is_empty()
            || !claims.insert(claim.claim_id.clone())
            || claim.study_id.trim().is_empty()
            || claim.subject.trim().is_empty()
            || claim.predicate.trim().is_empty()
            || claim.object.trim().is_empty()
            || claim.semantic_profile.trim().is_empty()
            || !canonical(&claim.omissions)
            || !canonical(&claim.uncertainty)
            || !digest_is_valid(&claim.artifact_digest)
            || !digest_is_valid(&claim.provenance_digest)
            || !digest_is_valid(&claim.replay_identity)
        {
            return Err(invalid(format!(
                "claim {} is malformed or duplicated",
                claim.claim_id
            )));
        }
    }
    let mut peers = BTreeSet::new();
    for peer in &request.peers {
        if peer.institution_id.trim().is_empty()
            || !peers.insert(peer.institution_id.clone())
            || peer.semantic_profile.trim().is_empty()
            || !digest_is_valid(&peer.knowledge_digest)
            || !digest_is_valid(&peer.replay_identity)
        {
            return Err(invalid(format!(
                "peer {} is malformed or duplicated",
                peer.institution_id
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
    fn request() -> KnowledgeRepresentationRequest {
        let d = hash("knowledge");
        let claim = KnowledgeClaimAttestation {
            claim_id: "claim:one".into(),
            study_id: "study:one".into(),
            subject: "gene:a".into(),
            predicate: "associated-with".into(),
            object: "phenotype:b".into(),
            semantic_profile: "profile:v1".into(),
            evidence_state: EvidenceState::Supported,
            artifact_digest: d.clone(),
            provenance_digest: d.clone(),
            replay_identity: d.clone(),
            omissions: vec![],
            uncertainty: vec![],
            negative_result: false,
            local_only: true,
            permitted: true,
        };
        let peer = |id: &str| PeerKnowledgeSummary {
            institution_id: id.into(),
            knowledge_digest: d.clone(),
            semantic_profile: "profile:v1".into(),
            replay_identity: d.clone(),
            signed: true,
            permitted: true,
            aggregate_only: true,
        };
        KnowledgeRepresentationRequest {
            schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
            request_id: "knowledge:one".into(),
            federation_id: "fed:knowledge".into(),
            semantic_profile: "profile:v1".into(),
            required_claim_order: vec!["claim:one".into()],
            claims: vec![claim],
            peers: vec![peer("inst:a"), peer("inst:b")],
            minimum_peer_quorum: 2,
            replay_identity: d,
            policy_allow: true,
            protected_closure: true,
            federation_approved: true,
            raw_data_local: true,
            aggregate_only: true,
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }
    #[test]
    fn manifest_is_a1() {
        assert_eq!(
            knowledge_representation_contract_manifest().autonomy_tier,
            AutonomyTier::A1
        );
    }
    #[test]
    fn qualified() {
        assert_eq!(
            model_knowledge_representation_contract(&request())
                .unwrap()
                .disposition,
            KnowledgeWorldDisposition::Qualified
        );
    }
    #[test]
    fn deterministic() {
        let a = model_knowledge_representation_contract(&request()).unwrap();
        let b = model_knowledge_representation_contract(&request()).unwrap();
        assert_eq!(a.world_digest, b.world_digest);
    }
    #[test]
    fn unknown_is_unresolved() {
        let mut value = request();
        value.claims[0].evidence_state = EvidenceState::Unknown;
        assert_eq!(
            model_knowledge_representation_contract(&value)
                .unwrap()
                .disposition,
            KnowledgeWorldDisposition::Unresolved
        );
    }
    #[test]
    fn contradiction_blocks() {
        let mut value = request();
        value.claims[0].evidence_state = EvidenceState::Contradicted;
        assert_eq!(
            model_knowledge_representation_contract(&value)
                .unwrap()
                .disposition,
            KnowledgeWorldDisposition::Blocked
        );
    }
    #[test]
    fn quorum_unresolved() {
        let mut value = request();
        value.peers[0].signed = false;
        assert_eq!(
            model_knowledge_representation_contract(&value)
                .unwrap()
                .disposition,
            KnowledgeWorldDisposition::Unresolved
        );
    }
    #[test]
    fn policy_blocks() {
        let mut value = request();
        value.policy_allow = false;
        assert_eq!(
            model_knowledge_representation_contract(&value)
                .unwrap()
                .disposition,
            KnowledgeWorldDisposition::Blocked
        );
    }
}
