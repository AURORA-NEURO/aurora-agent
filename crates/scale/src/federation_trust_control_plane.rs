//! Federated trust-envelope control plane for `AFA-scale-P20-F32`.
//!
//! The plane evaluates institution-local declarations and prepares a digest-bound exchange
//! envelope.  It does not move bytes, authenticate identities, contact peers, or assert that a
//! scientific result is true.  Every missing, stale, contradictory, revoked, or adversarial
//! condition remains visible and any unsafe global posture fails closed.

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

pub const FEATURE_ID: &str = "AFA-scale-P20-F32";
pub const CONTRACT_VERSION: &str =
    "scale-federated-continual-security-federation-control-plane/1.0";
pub const INPUT_SCHEMA: &str = "FederationRequest4@1";
pub const OUTPUT_SCHEMA: &str = "FederationEnvelope8@1";
pub const CONTENT_TYPE: &str = "application/vnd.aurora.scale-federation-envelope-8+json";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederationPeer7 {
    pub peer_id: String,
    pub institution_id: String,
    pub semantic_profile: String,
    pub offered_artifact_order: Vec<String>,
    pub capability_digest: ContentHash,
    pub provenance_digest: ContentHash,
    pub evidence_digest: ContentHash,
    pub replay_identity: ContentHash,
    pub evidence_state: EvidenceState,
    pub signed: bool,
    pub policy_permitted: bool,
    pub purpose_allowed: bool,
    pub aggregate_only: bool,
    pub raw_data_local: bool,
    pub revoked: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederationRequest4 {
    pub schema_version: String,
    pub request_id: String,
    pub origin_institution: String,
    pub purpose: String,
    pub semantic_profile: String,
    pub requested_artifact_order: Vec<String>,
    pub peers: Vec<FederationPeer7>,
    pub replay_identity: ContentHash,
    pub policy_allow: bool,
    pub protected_closure: bool,
    pub signed_approval: bool,
    pub authority_present: bool,
    pub federation_approved: bool,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub adversarial_events: Vec<String>,
    pub boundary: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FederationDisposition {
    Qualified,
    Unresolved,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederationEnvelope8 {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub origin_institution: String,
    pub purpose: String,
    pub semantic_profile: String,
    pub disposition: FederationDisposition,
    pub requested_artifact_order: Vec<String>,
    pub peer_order: Vec<String>,
    pub selected_peer_order: Vec<String>,
    pub unresolved_peer_order: Vec<String>,
    pub blocked_peer_order: Vec<String>,
    pub qualified_artifact_order: Vec<String>,
    pub missing_artifact_order: Vec<String>,
    pub omission_order: Vec<String>,
    pub uncertainty_order: Vec<String>,
    pub negative_evidence_order: Vec<String>,
    pub federation_digest: ContentHash,
    pub artifact: TypedResearchArtifact,
    pub effect_receipts: Vec<String>,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub boundary: String,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum FederationTrustError {
    #[error("invalid federation request: {0}")]
    Invalid(String),
    #[error("federation envelope artifact failed: {0}")]
    Artifact(String),
}

fn invalid(message: impl Into<String>) -> FederationTrustError {
    FederationTrustError::Invalid(message.into())
}

fn canonical(values: &[String]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn digest(value: &ContentHash) -> bool {
    value.as_str().len() == 64
}

impl FederationEnvelope8 {
    pub fn validate(&self) -> Result<(), FederationTrustError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.boundary != PRECLINICAL_BOUNDARY
            || self.artifact.boundary != PRECLINICAL_BOUNDARY
            || !self.raw_data_local
            || !self.aggregate_only
            || self.request_id.trim().is_empty()
            || self.origin_institution.trim().is_empty()
            || self.purpose.trim().is_empty()
            || self.semantic_profile.trim().is_empty()
            || self.requested_artifact_order.is_empty()
            || self.peer_order.is_empty()
            || self.effect_receipts.is_empty()
        {
            return Err(invalid(
                "federation identity, artifacts, peers, locality, or effects are incomplete",
            ));
        }
        for values in [
            &self.requested_artifact_order,
            &self.peer_order,
            &self.selected_peer_order,
            &self.unresolved_peer_order,
            &self.blocked_peer_order,
            &self.qualified_artifact_order,
            &self.missing_artifact_order,
            &self.omission_order,
            &self.uncertainty_order,
            &self.negative_evidence_order,
            &self.effect_receipts,
        ] {
            if !canonical(values) {
                return Err(invalid("federation ordering is not canonical"));
            }
        }
        let peers = self.peer_order.iter().cloned().collect::<BTreeSet<_>>();
        let peer_parts = self
            .selected_peer_order
            .iter()
            .chain(self.unresolved_peer_order.iter())
            .chain(self.blocked_peer_order.iter())
            .cloned()
            .collect::<Vec<_>>();
        if peers.len() != self.peer_order.len()
            || peer_parts.len() != peers.len()
            || peer_parts.iter().cloned().collect::<BTreeSet<_>>() != peers
        {
            return Err(invalid("peer states do not form a complete partition"));
        }
        let artifacts = self
            .requested_artifact_order
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>();
        let artifact_parts = self
            .qualified_artifact_order
            .iter()
            .chain(self.missing_artifact_order.iter())
            .cloned()
            .collect::<Vec<_>>();
        if artifacts.len() != self.requested_artifact_order.len()
            || artifact_parts.len() != artifacts.len()
            || artifact_parts.iter().cloned().collect::<BTreeSet<_>>() != artifacts
        {
            return Err(invalid("artifact coverage is not a complete partition"));
        }
        for value in [&self.federation_digest, &self.artifact.content_hash] {
            if !digest(value) {
                return Err(invalid("federation digest is invalid"));
            }
        }
        self.artifact
            .validate_metadata()
            .map_err(|error| FederationTrustError::Artifact(error.to_string()))?;
        if self.artifact.content_type != CONTENT_TYPE
            || self.artifact.content_hash != self.federation_digest
        {
            return Err(invalid(
                "federation artifact metadata or digest is inconsistent",
            ));
        }
        if self.effect_receipts.iter().any(|effect| {
            !effect.starts_with("exchange:permitted-summaries:")
                && !effect.starts_with("manage:local-capability:")
                && effect != "block:unsafe-release"
        }) {
            return Err(invalid("effect is outside the federation gate"));
        }
        if self.disposition == FederationDisposition::Qualified
            && self.effect_receipts
                != [
                    format!("exchange:permitted-summaries:{}", self.request_id),
                    format!("manage:local-capability:{}", self.request_id),
                ]
        {
            return Err(invalid("qualified federation effects are invalid"));
        }
        if self.disposition != FederationDisposition::Qualified
            && self.effect_receipts != ["block:unsafe-release"]
        {
            return Err(invalid("non-qualified federation must block release"));
        }
        Ok(())
    }

    pub fn digest(&self) -> Result<ContentHash, FederationTrustError> {
        self.validate()?;
        ContentHash::of_value(
            &serde_json::to_value(self)
                .map_err(|error| FederationTrustError::Artifact(error.to_string()))?,
        )
        .map_err(|error| FederationTrustError::Artifact(error.to_string()))
    }
}

pub fn federation_trust_control_plane_manifest() -> CapabilityManifest {
    CapabilityManifest {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        capability_id: FEATURE_ID.into(),
        version: CONTRACT_VERSION.into(),
        owner_crate: "scale".into(),
        consumers: [
            "platform reliability engineer".into(),
            "federation operator".into(),
            "downstream research workflow".into(),
        ]
        .into(),
        behavior: "evaluates institution-local federation trust declarations and prepares a minimal signed-envelope intent under policy, provenance, replay, capacity, and locality gates without moving raw data".into(),
        value: "turns continual multi-institution exchange into an auditable, fail-closed control-plane product".into(),
        inputs: vec![TypedPort {
            name: "federation_request".into(),
            schema: INPUT_SCHEMA.into(),
            required: true,
        }],
        outputs: vec![TypedPort {
            name: "federation_envelope".into(),
            schema: OUTPUT_SCHEMA.into(),
            required: true,
        }],
        effects: [
            Effect::ExecuteLocalComputation,
            Effect::WriteLocalArtifact,
            Effect::FederationExport,
        ]
        .into(),
        permissions: ["operate:institution-node".into(), "exchange:permitted-summaries".into()].into(),
        determinism: Determinism::ByteStable,
        evidence: vec![
            EvidenceReference {
                source_id: "w3c-prov-o".into(),
                state: EvidenceState::Supported,
                locator: Some("https://www.w3.org/TR/prov-o/".into()),
            },
            EvidenceReference {
                source_id: "ro-crate-1.3".into(),
                state: EvidenceState::Supported,
                locator: Some("https://www.researchobject.org/ro-crate/".into()),
            },
            EvidenceReference {
                source_id: "ga4gh-drs-1.3".into(),
                state: EvidenceState::Supported,
                locator: Some("https://ga4gh.github.io/data-repository-service-schemas/".into()),
            },
            EvidenceReference {
                source_id: "ome-ngff-rfc5".into(),
                state: EvidenceState::Supported,
                locator: Some("https://ngff.openmicroscopy.org/rfc/5/".into()),
            },
        ],
        authority_requirements: vec![
            AuthorityRequirement {
                role: "federation steward".into(),
                reason: "approve purpose-bounded institution exchange".into(),
            },
            AuthorityRequirement {
                role: "platform reliability engineer".into(),
                reason: "operate local control-plane budget and recovery".into(),
            },
        ],
        autonomy_tier: AutonomyTier::A2,
        surfaces: [
            ResearchSurface::Ui,
            ResearchSurface::Api,
            ResearchSurface::Sdk,
            ResearchSurface::McpTool,
            ResearchSurface::Policy,
            ResearchSurface::Operator,
        ]
        .into(),
        boundary: PRECLINICAL_BOUNDARY.into(),
    }
}

pub fn assure_federation(
    request: &FederationRequest4,
) -> Result<FederationEnvelope8, FederationTrustError> {
    validate_request(request)?;
    let mut peers = request.peers.clone();
    peers.sort_by(|left, right| left.peer_id.cmp(&right.peer_id));
    let peer_order = peers
        .iter()
        .map(|peer| peer.peer_id.clone())
        .collect::<Vec<_>>();
    let mut selected = BTreeSet::new();
    let mut unresolved = BTreeSet::new();
    let mut blocked = BTreeSet::new();
    let mut omissions = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    let mut negative = BTreeSet::new();
    let mut qualified_artifacts = BTreeSet::new();
    for peer in &peers {
        if peer.revoked || !peer.policy_permitted || !peer.purpose_allowed {
            blocked.insert(peer.peer_id.clone());
            if peer.revoked {
                negative.insert(format!("{}:revoked", peer.peer_id));
            }
            if !peer.policy_permitted {
                negative.insert(format!("{}:policy-denied", peer.peer_id));
            }
            if !peer.purpose_allowed {
                uncertainty.insert(format!("{}:purpose-not-authorized", peer.peer_id));
            }
        } else if !peer.raw_data_local
            || !peer.aggregate_only
            || !peer.signed
            || peer.semantic_profile != request.semantic_profile
            || peer.replay_identity != request.replay_identity
            || !matches!(
                peer.evidence_state,
                EvidenceState::Proven | EvidenceState::Supported
            )
        {
            unresolved.insert(peer.peer_id.clone());
            if peer.semantic_profile != request.semantic_profile {
                uncertainty.insert(format!("{}:semantic-profile-mismatch", peer.peer_id));
            }
            if peer.replay_identity != request.replay_identity {
                uncertainty.insert(format!("{}:replay-mismatch", peer.peer_id));
            }
            if !peer.signed {
                uncertainty.insert(format!("{}:signature-missing", peer.peer_id));
            }
            if !peer.raw_data_local || !peer.aggregate_only {
                omissions.insert(format!("{}:raw-data-not-local-or-aggregate", peer.peer_id));
            }
            if !matches!(
                peer.evidence_state,
                EvidenceState::Proven | EvidenceState::Supported
            ) {
                uncertainty.insert(format!("{}:evidence-state", peer.peer_id));
            }
        } else {
            selected.insert(peer.peer_id.clone());
            qualified_artifacts.extend(peer.offered_artifact_order.iter().cloned());
        }
    }
    let requested = request
        .requested_artifact_order
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let missing_artifacts = requested
        .difference(&qualified_artifacts)
        .cloned()
        .collect::<BTreeSet<_>>();
    if !missing_artifacts.is_empty() {
        omissions.extend(
            missing_artifacts
                .iter()
                .map(|artifact| format!("{artifact}:qualified-peer-coverage-missing")),
        );
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
    if !request.authority_present {
        uncertainty.insert("request:institution-authority-missing".into());
    }
    if !request.federation_approved {
        uncertainty.insert("request:federation-approval-missing".into());
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
        || !request.authority_present
        || !request.federation_approved
        || !request.raw_data_local
        || !request.aggregate_only
        || !request.adversarial_events.is_empty();
    if global_block {
        blocked.extend(peer_order.iter().cloned());
        selected.clear();
        unresolved.clear();
        qualified_artifacts.clear();
        omissions.insert("request:federation-release-gate-blocked".into());
    }
    let disposition = if global_block || selected.is_empty() && !blocked.is_empty() {
        FederationDisposition::Blocked
    } else if selected.is_empty() || !missing_artifacts.is_empty() || !unresolved.is_empty() {
        FederationDisposition::Unresolved
    } else {
        FederationDisposition::Qualified
    };
    if disposition != FederationDisposition::Qualified {
        omissions.insert("request:federation-envelope-not-release-ready".into());
    }
    let selected_peer_order = selected.into_iter().collect::<Vec<_>>();
    let unresolved_peer_order = unresolved.into_iter().collect::<Vec<_>>();
    let blocked_peer_order = blocked.into_iter().collect::<Vec<_>>();
    let qualified_artifact_order = qualified_artifacts
        .intersection(&requested)
        .cloned()
        .collect::<Vec<_>>();
    let missing_artifact_order = requested
        .difference(&qualified_artifacts)
        .cloned()
        .collect::<Vec<_>>();
    let omission_order = omissions.into_iter().collect::<Vec<_>>();
    let uncertainty_order = uncertainty.into_iter().collect::<Vec<_>>();
    let negative_evidence_order = negative.into_iter().collect::<Vec<_>>();
    let effect_receipts = if disposition == FederationDisposition::Qualified {
        vec![
            format!("exchange:permitted-summaries:{}", request.request_id),
            format!("manage:local-capability:{}", request.request_id),
        ]
    } else {
        vec!["block:unsafe-release".into()]
    };
    let payload = json!({
        "schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION,
        "contract_version": CONTRACT_VERSION,
        "feature_id": FEATURE_ID,
        "request_id": request.request_id,
        "origin_institution": request.origin_institution,
        "purpose": request.purpose,
        "semantic_profile": request.semantic_profile,
        "disposition": disposition,
        "requested_artifact_order": request.requested_artifact_order,
        "peer_order": peer_order,
        "selected_peer_order": selected_peer_order,
        "unresolved_peer_order": unresolved_peer_order,
        "blocked_peer_order": blocked_peer_order,
        "qualified_artifact_order": qualified_artifact_order,
        "missing_artifact_order": missing_artifact_order,
        "omission_order": omission_order,
        "uncertainty_order": uncertainty_order,
        "negative_evidence_order": negative_evidence_order,
        "effect_receipts": effect_receipts,
        "raw_data_local": request.raw_data_local,
        "aggregate_only": request.aggregate_only,
        "boundary": PRECLINICAL_BOUNDARY,
    });
    let federation_digest = ContentHash::of_value(&payload)
        .map_err(|error| FederationTrustError::Artifact(error.to_string()))?;
    let artifact = TypedResearchArtifact::from_payload(
        format!("scale-federation-envelope:{}", request.request_id),
        CONTENT_TYPE,
        &payload,
        Vec::new(),
        Vec::new(),
    )
    .map_err(|error| FederationTrustError::Artifact(error.to_string()))?;
    let receipt = FederationEnvelope8 {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        request_id: request.request_id.clone(),
        origin_institution: request.origin_institution.clone(),
        purpose: request.purpose.clone(),
        semantic_profile: request.semantic_profile.clone(),
        disposition,
        requested_artifact_order: payload["requested_artifact_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|value| value.as_str().unwrap().into())
            .collect(),
        peer_order: payload["peer_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|value| value.as_str().unwrap().into())
            .collect(),
        selected_peer_order: payload["selected_peer_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|value| value.as_str().unwrap().into())
            .collect(),
        unresolved_peer_order: payload["unresolved_peer_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|value| value.as_str().unwrap().into())
            .collect(),
        blocked_peer_order: payload["blocked_peer_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|value| value.as_str().unwrap().into())
            .collect(),
        qualified_artifact_order: payload["qualified_artifact_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|value| value.as_str().unwrap().into())
            .collect(),
        missing_artifact_order: payload["missing_artifact_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|value| value.as_str().unwrap().into())
            .collect(),
        omission_order: payload["omission_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|value| value.as_str().unwrap().into())
            .collect(),
        uncertainty_order: payload["uncertainty_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|value| value.as_str().unwrap().into())
            .collect(),
        negative_evidence_order: payload["negative_evidence_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|value| value.as_str().unwrap().into())
            .collect(),
        federation_digest,
        artifact,
        effect_receipts: payload["effect_receipts"]
            .as_array()
            .unwrap()
            .iter()
            .map(|value| value.as_str().unwrap().into())
            .collect(),
        raw_data_local: request.raw_data_local,
        aggregate_only: request.aggregate_only,
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    receipt.validate()?;
    Ok(receipt)
}

fn validate_request(request: &FederationRequest4) -> Result<(), FederationTrustError> {
    if request.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
        || request.request_id.trim().is_empty()
        || request.origin_institution.trim().is_empty()
        || request.purpose.trim().is_empty()
        || request.semantic_profile.trim().is_empty()
        || request.requested_artifact_order.is_empty()
        || !canonical(&request.requested_artifact_order)
        || request.peers.is_empty()
        || !canonical(&request.adversarial_events)
        || !digest(&request.replay_identity)
        || request.boundary != PRECLINICAL_BOUNDARY
        || !request.raw_data_local
        || !request.aggregate_only
    {
        return Err(invalid(
            "federation identity, artifact order, peers, replay, locality, or boundary is invalid",
        ));
    }
    let mut ids = BTreeSet::new();
    for peer in &request.peers {
        if peer.peer_id.trim().is_empty()
            || peer.institution_id.trim().is_empty()
            || peer.semantic_profile.trim().is_empty()
            || !ids.insert(peer.peer_id.clone())
            || peer.offered_artifact_order.is_empty()
            || !canonical(&peer.offered_artifact_order)
            || !digest(&peer.capability_digest)
            || !digest(&peer.provenance_digest)
            || !digest(&peer.evidence_digest)
            || !digest(&peer.replay_identity)
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

    fn request() -> FederationRequest4 {
        let peer = |id: &str, artifact: &str| FederationPeer7 {
            peer_id: id.into(),
            institution_id: format!("institution:{id}"),
            semantic_profile: "preclinical:envelope:v1".into(),
            offered_artifact_order: vec![artifact.into()],
            capability_digest: hash("capability"),
            provenance_digest: hash("provenance"),
            evidence_digest: hash("evidence"),
            replay_identity: hash("replay"),
            evidence_state: EvidenceState::Supported,
            signed: true,
            policy_permitted: true,
            purpose_allowed: true,
            aggregate_only: true,
            raw_data_local: true,
            revoked: false,
        };
        FederationRequest4 {
            schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
            request_id: "request:federation".into(),
            origin_institution: "institution:origin".into(),
            purpose: "preclinical-multimodal-benchmark".into(),
            semantic_profile: "preclinical:envelope:v1".into(),
            requested_artifact_order: vec!["artifact:a".into(), "artifact:b".into()],
            peers: vec![peer("peer:a", "artifact:a"), peer("peer:b", "artifact:b")],
            replay_identity: hash("replay"),
            policy_allow: true,
            protected_closure: true,
            signed_approval: true,
            authority_present: true,
            federation_approved: true,
            raw_data_local: true,
            aggregate_only: true,
            adversarial_events: Vec::new(),
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }

    #[test]
    fn manifest_is_a2() {
        assert_eq!(
            federation_trust_control_plane_manifest().autonomy_tier,
            AutonomyTier::A2
        );
    }

    #[test]
    fn qualified_coverage_is_deterministic() {
        let envelope = assure_federation(&request()).unwrap();
        assert_eq!(envelope.disposition, FederationDisposition::Qualified);
        assert_eq!(envelope.qualified_artifact_order.len(), 2);
        assert_eq!(envelope.digest().unwrap(), envelope.digest().unwrap());
    }

    #[test]
    fn missing_artifact_coverage_is_unresolved() {
        let mut request = request();
        request.peers[1].offered_artifact_order = vec!["artifact:c".into()];
        let envelope = assure_federation(&request).unwrap();
        assert_eq!(envelope.disposition, FederationDisposition::Unresolved);
        assert!(envelope
            .missing_artifact_order
            .contains(&"artifact:b".into()));
    }

    #[test]
    fn revoked_peer_is_blocked_and_negative() {
        let mut request = request();
        request.peers[0].revoked = true;
        let envelope = assure_federation(&request).unwrap();
        assert_eq!(envelope.blocked_peer_order, vec!["peer:a"]);
        assert!(envelope
            .negative_evidence_order
            .contains(&"peer:a:revoked".into()));
    }

    #[test]
    fn adversarial_request_blocks_every_peer() {
        let mut request = request();
        request.adversarial_events = vec!["poisoned-envelope".into()];
        let envelope = assure_federation(&request).unwrap();
        assert_eq!(envelope.disposition, FederationDisposition::Blocked);
        assert!(envelope.selected_peer_order.is_empty());
        assert_eq!(envelope.blocked_peer_order.len(), 2);
    }

    #[test]
    fn semantic_mismatch_is_unresolved() {
        let mut request = request();
        request.peers[0].semantic_profile = "other-profile".into();
        let envelope = assure_federation(&request).unwrap();
        assert_eq!(envelope.disposition, FederationDisposition::Unresolved);
        assert!(envelope
            .uncertainty_order
            .contains(&"peer:a:semantic-profile-mismatch".into()));
    }

    #[test]
    fn policy_denial_fails_closed() {
        let mut request = request();
        request.policy_allow = false;
        let envelope = assure_federation(&request).unwrap();
        assert_eq!(envelope.disposition, FederationDisposition::Blocked);
        assert_eq!(envelope.effect_receipts, vec!["block:unsafe-release"]);
    }
}
