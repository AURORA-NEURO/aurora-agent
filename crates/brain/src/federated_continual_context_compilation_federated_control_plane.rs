//! Federated-continual context compilation operations and federation control plane.
//!
//! Atlas feature: `AFA-brain-P03-F32`. Peer freshness, semantic comparability,
//! quorum, locality, signed approval, and aggregate-only exchange are explicit.

use bioprism_foundation::{
    AutonomyTier, CapabilityManifest, Determinism, Effect, EvidenceReference, EvidenceState,
    ResearchSurface, TypedPort, TypedResearchArtifact, PRECLINICAL_BOUNDARY,
    RESEARCH_CONTRACT_SCHEMA_VERSION,
};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-brain-P03-F32";
pub const CONTRACT_VERSION: &str =
    "brain-federated-continual-context-compilation-federated-control-plane/1.0";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederatedContinualContextControlPeer {
    pub peer_id: String,
    pub institution_id: String,
    pub context_digest: ContentHash,
    pub section_digest: ContentHash,
    pub evidence_digest: Option<ContentHash>,
    pub provenance_digest: Option<ContentHash>,
    pub replay_identity: ContentHash,
    pub semantic_profile: String,
    pub fresh: bool,
    pub comparable: bool,
    pub permitted_summary: bool,
    pub signed_approval: bool,
    pub policy_allow: bool,
    pub ready: bool,
    pub state: EvidenceState,
    pub raw_data_local: bool,
    pub boundary: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederatedContinualContextControlRequest {
    pub request_id: String,
    pub federation_id: String,
    pub round_id: String,
    pub peers: Vec<FederatedContinualContextControlPeer>,
    pub min_quorum: u16,
    pub replay_identity: ContentHash,
    pub policy_allow: bool,
    pub protected_closure: bool,
    pub raw_data_local: bool,
    pub signed_approval: bool,
    pub boundary: String,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FederatedContinualContextControlDisposition {
    Completed,
    Degraded,
    Unresolved,
    Denied,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederatedContinualContextControlReceipt {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub federation_id: String,
    pub round_id: String,
    pub disposition: FederatedContinualContextControlDisposition,
    pub candidate_order: Vec<String>,
    pub qualified_order: Vec<String>,
    pub degraded_order: Vec<String>,
    pub unresolved_order: Vec<String>,
    pub denied_order: Vec<String>,
    pub exchange_order: Vec<ContentHash>,
    pub semantic_profile_order: Vec<String>,
    pub freshness_order: Vec<String>,
    pub checkpoint_seq: u64,
    pub quorum_required: u16,
    pub quorum_met: bool,
    pub run_digest: ContentHash,
    pub telemetry_digest: ContentHash,
    pub federation_digest: ContentHash,
    pub replay_identity: ContentHash,
    pub witness_order: Vec<String>,
    pub counterexample_order: Vec<String>,
    pub omissions: Vec<String>,
    pub uncertainty: Vec<String>,
    pub negative_evidence: Vec<String>,
    pub effect_receipts: Vec<String>,
    pub artifact: TypedResearchArtifact,
    pub raw_data_local: bool,
    pub boundary: String,
}
#[derive(Debug, Error, PartialEq, Eq)]
pub enum FederatedContinualContextControlError {
    #[error("invalid federated continual context control request: {0}")]
    Invalid(String),
    #[error("federated continual context control artifact failed: {0}")]
    Artifact(String),
}

impl FederatedContinualContextControlReceipt {
    pub fn validate(&self) -> Result<(), FederatedContinualContextControlError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.boundary != PRECLINICAL_BOUNDARY
            || !self.raw_data_local
            || self.request_id.trim().is_empty()
            || self.federation_id.trim().is_empty()
            || self.round_id.trim().is_empty()
            || self.candidate_order.is_empty()
            || self.checkpoint_seq != self.candidate_order.len() as u64
            || self.quorum_required == 0
            || self.effect_receipts.is_empty()
        {
            return Err(FederatedContinualContextControlError::Invalid("federated continual identity, checkpoint, quorum, locality, or effects are incomplete".into()));
        }
        for values in [
            &self.candidate_order,
            &self.qualified_order,
            &self.degraded_order,
            &self.unresolved_order,
            &self.denied_order,
            &self.semantic_profile_order,
            &self.freshness_order,
            &self.witness_order,
            &self.counterexample_order,
            &self.omissions,
            &self.uncertainty,
            &self.negative_evidence,
            &self.effect_receipts,
        ] {
            if values.windows(2).any(|pair| pair[0] >= pair[1]) {
                return Err(FederatedContinualContextControlError::Invalid(
                    "federated continual ordering is not canonical".into(),
                ));
            }
        }
        if self
            .exchange_order
            .windows(2)
            .any(|pair| pair[0] >= pair[1])
        {
            return Err(FederatedContinualContextControlError::Invalid(
                "federated continual exchange ordering is not canonical".into(),
            ));
        }
        let classified = self
            .qualified_order
            .iter()
            .chain(self.degraded_order.iter())
            .chain(self.unresolved_order.iter())
            .chain(self.denied_order.iter())
            .cloned()
            .collect::<BTreeSet<_>>();
        if classified.len() != self.candidate_order.len()
            || classified
                .iter()
                .any(|peer| !self.candidate_order.contains(peer))
        {
            return Err(FederatedContinualContextControlError::Invalid(
                "federated continual dispositions do not partition peers".into(),
            ));
        }
        if self.exchange_order.len() != self.qualified_order.len() {
            return Err(FederatedContinualContextControlError::Invalid(
                "federated continual exchange does not match qualified peers".into(),
            ));
        }
        for digest in self.exchange_order.iter().chain([
            &self.run_digest,
            &self.telemetry_digest,
            &self.federation_digest,
            &self.replay_identity,
        ]) {
            if digest.as_str().len() != 64 {
                return Err(FederatedContinualContextControlError::Invalid(
                    "federated continual digest is invalid".into(),
                ));
            }
        }
        if self.effect_receipts.iter().any(|effect| {
            !effect.starts_with("exchange:permitted-federated-context-summary:")
                && !effect.starts_with("manage:federated-context:")
                && effect != "block:unsafe-release"
        }) {
            return Err(FederatedContinualContextControlError::Invalid(
                "federated continual effect is outside the governed operations gate".into(),
            ));
        }
        self.artifact
            .validate_metadata()
            .map_err(|error| FederatedContinualContextControlError::Artifact(error.to_string()))
    }
    pub fn digest(&self) -> Result<ContentHash, FederatedContinualContextControlError> {
        self.validate()?;
        let value = serde_json::to_value(self)
            .map_err(|error| FederatedContinualContextControlError::Artifact(error.to_string()))?;
        ContentHash::of_value(&value)
            .map_err(|error| FederatedContinualContextControlError::Artifact(error.to_string()))
    }
}

pub fn federated_continual_context_compilation_federated_control_plane_manifest(
) -> CapabilityManifest {
    CapabilityManifest { schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(), capability_id: FEATURE_ID.into(), version: CONTRACT_VERSION.into(), owner_crate: "brain".into(), consumers: ["agent developer".into(), "federation administrator".into(), "independent validation partner".into()].into(), behavior: "operates federated continual context rounds with freshness, semantic comparability, quorum, signed approvals, and aggregate-only permitted-summary exchange".into(), value: "prevents stale or semantically incompatible peer context from becoming a qualified decision section while preserving partial and negative federation evidence".into(), inputs: vec![TypedPort { name: "federated_continual_context_control_request".into(), schema: "FederatedContinualContextControlRequest1@1".into(), required: true }], outputs: vec![TypedPort { name: "federated_continual_context_control_receipt".into(), schema: "FederatedContinualContextControlReceipt1@1".into(), required: true }], effects: [Effect::ReadLocalData, Effect::ExecuteLocalComputation, Effect::WriteLocalArtifact, Effect::FederationExport].into(), permissions: ["operate:institution-node".into()].into(), determinism: Determinism::ByteStable, evidence: vec![EvidenceReference { source_id: "opentelemetry".into(), state: EvidenceState::Supported, locator: Some("https://opentelemetry.io/docs/specs/".into()) }], authority_requirements: Vec::new(), autonomy_tier: AutonomyTier::A2, surfaces: [ResearchSurface::Ui, ResearchSurface::Api, ResearchSurface::Sdk, ResearchSurface::Cli, ResearchSurface::McpTool, ResearchSurface::Policy, ResearchSurface::Operator].into(), boundary: PRECLINICAL_BOUNDARY.into() }
}

pub fn operate_federated_continual_context_compilation(
    request: &FederatedContinualContextControlRequest,
) -> Result<FederatedContinualContextControlReceipt, FederatedContinualContextControlError> {
    if request.request_id.trim().is_empty()
        || request.federation_id.trim().is_empty()
        || request.round_id.trim().is_empty()
        || request.peers.is_empty()
        || request.min_quorum == 0
        || request.min_quorum as usize > request.peers.len()
        || request.replay_identity.as_str().len() != 64
        || request.boundary != PRECLINICAL_BOUNDARY
    {
        return Err(FederatedContinualContextControlError::Invalid(
            "federated continual identity, peer set, quorum, replay, or boundary is invalid".into(),
        ));
    }
    let mut peers = request.peers.clone();
    peers.sort_by(|left, right| left.peer_id.cmp(&right.peer_id));
    let candidate = peers
        .iter()
        .map(|peer| peer.peer_id.clone())
        .collect::<Vec<_>>();
    if candidate.windows(2).any(|pair| pair[0] == pair[1])
        || candidate.iter().any(|value| value.trim().is_empty())
    {
        return Err(FederatedContinualContextControlError::Invalid(
            "federated continual peer identifiers must be unique and non-empty".into(),
        ));
    }
    let peer_map = peers
        .iter()
        .map(|peer| (peer.peer_id.clone(), peer))
        .collect::<BTreeMap<_, _>>();
    let mut qualified = BTreeSet::new();
    let degraded = BTreeSet::new();
    let mut unresolved = BTreeSet::new();
    let mut denied = BTreeSet::new();
    let mut exchanges = Vec::new();
    let mut semantic_profiles = BTreeSet::new();
    let mut fresh = BTreeSet::new();
    let mut witnesses = BTreeSet::from([
        "gate:typed-federated-context-contract".to_string(),
        "gate:freshness".to_string(),
        "gate:semantic-comparability".to_string(),
        "gate:quorum".to_string(),
        "gate:replay-identity".to_string(),
        "gate:aggregate-only".to_string(),
        "gate:locality".to_string(),
    ]);
    let mut counterexamples = BTreeSet::new();
    let mut omissions = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    let mut negative = BTreeSet::new();
    let global_open = request.policy_allow
        && request.protected_closure
        && request.raw_data_local
        && request.signed_approval;
    for peer_id in &candidate {
        let peer = peer_map[peer_id];
        if !global_open
            || !peer.policy_allow
            || !peer.signed_approval
            || !peer.permitted_summary
            || !peer.raw_data_local
            || peer.boundary != PRECLINICAL_BOUNDARY
        {
            denied.insert(peer_id.clone());
            counterexamples.insert(format!(
                "counterexample:{}:policy-approval-locality-or-purpose",
                peer_id
            ));
        } else if !peer.fresh {
            unresolved.insert(peer_id.clone());
            omissions.insert(format!("peer:{}:stale-context", peer_id));
        } else if !peer.comparable || peer.semantic_profile.trim().is_empty() {
            denied.insert(peer_id.clone());
            counterexamples.insert(format!(
                "counterexample:{}:semantic-profile-mismatch",
                peer_id
            ));
        } else if !peer.ready {
            unresolved.insert(peer_id.clone());
            uncertainty.insert(format!("peer:{}:not-ready", peer_id));
        } else if peer.replay_identity != request.replay_identity {
            unresolved.insert(peer_id.clone());
            uncertainty.insert(format!("peer:{}:replay-mismatch", peer_id));
        } else if peer.evidence_digest.is_none() || peer.provenance_digest.is_none() {
            unresolved.insert(peer_id.clone());
            omissions.insert(format!("peer:{}:evidence-or-provenance-missing", peer_id));
        } else if matches!(
            peer.state,
            EvidenceState::Unknown | EvidenceState::Speculative
        ) {
            unresolved.insert(peer_id.clone());
            uncertainty.insert(format!("peer:{}:evidence-uncertain", peer_id));
        } else if matches!(peer.state, EvidenceState::Contradicted) {
            denied.insert(peer_id.clone());
            negative.insert(format!("peer:{}:contradicted", peer_id));
        } else {
            qualified.insert(peer_id.clone());
            semantic_profiles.insert(peer.semantic_profile.clone());
            fresh.insert(peer_id.clone());
            exchanges.push(ContentHash::of_value(&json!({"peer_id": peer.peer_id, "institution_id": peer.institution_id, "context_digest": peer.context_digest, "section_digest": peer.section_digest, "evidence_digest": peer.evidence_digest, "provenance_digest": peer.provenance_digest, "semantic_profile": peer.semantic_profile, "replay_identity": peer.replay_identity})).map_err(|error| FederatedContinualContextControlError::Artifact(error.to_string()))?);
        }
    }
    let quorum_met = qualified.len() >= request.min_quorum as usize;
    if !quorum_met {
        uncertainty.insert(format!(
            "quorum:required-{}:observed-{}",
            request.min_quorum,
            qualified.len()
        ));
        omissions.insert("federation:quorum-incomplete".into());
    }
    if !request.policy_allow {
        counterexamples.insert("counterexample:policy-denied".into());
        omissions.insert("control:policy-denied".into());
    }
    if !request.protected_closure {
        counterexamples.insert("counterexample:protected-closure-incomplete".into());
        omissions.insert("control:protected-closure-incomplete".into());
    }
    if !request.signed_approval {
        counterexamples.insert("counterexample:signed-approval-missing".into());
        omissions.insert("control:signed-approval-missing".into());
    }
    if !unresolved.is_empty() || !degraded.is_empty() {
        witnesses.insert("gate:partial-peer-results-retained".into());
    }
    exchanges.sort_by(|left, right| left.as_str().cmp(right.as_str()));
    let disposition = if !global_open || !denied.is_empty() {
        FederatedContinualContextControlDisposition::Denied
    } else if !quorum_met {
        FederatedContinualContextControlDisposition::Unresolved
    } else if !unresolved.is_empty() {
        FederatedContinualContextControlDisposition::Degraded
    } else {
        FederatedContinualContextControlDisposition::Completed
    };
    let telemetry = ContentHash::of_value(&json!({"feature_id": FEATURE_ID, "federation_id": request.federation_id, "round_id": request.round_id, "candidate_order": candidate, "qualified_order": qualified})).map_err(|error| FederatedContinualContextControlError::Artifact(error.to_string()))?;
    let federation = ContentHash::of_value(&json!({"federation_id": request.federation_id, "round_id": request.round_id, "exchange_order": exchanges, "quorum_required": request.min_quorum, "quorum_met": quorum_met, "raw_data_local": request.raw_data_local})).map_err(|error| FederatedContinualContextControlError::Artifact(error.to_string()))?;
    let run = ContentHash::of_value(&json!({"feature_id": FEATURE_ID, "request_id": request.request_id, "disposition": disposition, "qualified_order": qualified, "unresolved_order": unresolved, "denied_order": denied, "quorum_met": quorum_met, "telemetry_digest": telemetry, "federation_digest": federation, "replay_identity": request.replay_identity})).map_err(|error| FederatedContinualContextControlError::Artifact(error.to_string()))?;
    let payload = json!({"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "contract_version": CONTRACT_VERSION, "feature_id": FEATURE_ID, "request_id": request.request_id, "federation_id": request.federation_id, "round_id": request.round_id, "disposition": disposition, "candidate_order": candidate, "qualified_order": qualified, "degraded_order": degraded, "unresolved_order": unresolved, "denied_order": denied, "exchange_order": exchanges, "semantic_profile_order": semantic_profiles, "freshness_order": fresh, "checkpoint_seq": peers.len(), "quorum_required": request.min_quorum, "quorum_met": quorum_met, "run_digest": run, "telemetry_digest": telemetry, "federation_digest": federation, "replay_identity": request.replay_identity, "witness_order": witnesses, "counterexample_order": counterexamples, "omissions": omissions, "uncertainty": uncertainty, "negative_evidence": negative, "boundary": PRECLINICAL_BOUNDARY});
    let artifact = TypedResearchArtifact::from_payload(
        format!(
            "brain-federated-continual-context-compilation-federated-control-plane:{}",
            request.request_id
        ),
        "application/vnd.aurora.federated-continual-context-control+json",
        &payload,
        Vec::new(),
        Vec::new(),
    )
    .map_err(|error| FederatedContinualContextControlError::Artifact(error.to_string()))?;
    let receipt = FederatedContinualContextControlReceipt {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        request_id: request.request_id.clone(),
        federation_id: request.federation_id.clone(),
        round_id: request.round_id.clone(),
        disposition,
        candidate_order: candidate.clone(),
        qualified_order: qualified.into_iter().collect(),
        degraded_order: degraded.into_iter().collect(),
        unresolved_order: unresolved.into_iter().collect(),
        denied_order: denied.into_iter().collect(),
        exchange_order: exchanges,
        semantic_profile_order: semantic_profiles.into_iter().collect(),
        freshness_order: fresh.into_iter().collect(),
        checkpoint_seq: peers.len() as u64,
        quorum_required: request.min_quorum,
        quorum_met,
        run_digest: run,
        telemetry_digest: telemetry,
        federation_digest: federation,
        replay_identity: request.replay_identity.clone(),
        witness_order: witnesses.into_iter().collect(),
        counterexample_order: counterexamples.into_iter().collect(),
        omissions: omissions.into_iter().collect(),
        uncertainty: uncertainty.into_iter().collect(),
        negative_evidence: negative.into_iter().collect(),
        effect_receipts: if matches!(
            disposition,
            FederatedContinualContextControlDisposition::Completed
        ) {
            vec![
                format!(
                    "exchange:permitted-federated-context-summary:{}",
                    request.request_id
                ),
                format!("manage:federated-context:{}", request.request_id),
            ]
        } else {
            vec!["block:unsafe-release".into()]
        },
        artifact,
        raw_data_local: request.raw_data_local,
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    receipt.validate()?;
    Ok(receipt)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn hash(value: &str) -> ContentHash {
        ContentHash::of_bytes(value.as_bytes())
    }
    fn request() -> FederatedContinualContextControlRequest {
        let replay = hash("federated-continual-control");
        let peer = |id: &str| FederatedContinualContextControlPeer {
            peer_id: id.into(),
            institution_id: format!("institution:{}", id),
            context_digest: replay.clone(),
            section_digest: replay.clone(),
            evidence_digest: Some(replay.clone()),
            provenance_digest: Some(replay.clone()),
            replay_identity: replay.clone(),
            semantic_profile: "preclinical-v1".into(),
            fresh: true,
            comparable: true,
            permitted_summary: true,
            signed_approval: true,
            policy_allow: true,
            ready: true,
            state: EvidenceState::Supported,
            raw_data_local: true,
            boundary: PRECLINICAL_BOUNDARY.into(),
        };
        FederatedContinualContextControlRequest {
            request_id: "request:federated-continual-control".into(),
            federation_id: "federation:alpha".into(),
            round_id: "round:001".into(),
            peers: vec![peer("peer:a"), peer("peer:b")],
            min_quorum: 2,
            replay_identity: replay,
            policy_allow: true,
            protected_closure: true,
            raw_data_local: true,
            signed_approval: true,
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }
    #[test]
    fn manifest_is_a2() {
        assert_eq!(
            federated_continual_context_compilation_federated_control_plane_manifest()
                .autonomy_tier,
            AutonomyTier::A2
        );
    }
    #[test]
    fn quorum_is_completed() {
        assert_eq!(
            operate_federated_continual_context_compilation(&request())
                .unwrap()
                .disposition,
            FederatedContinualContextControlDisposition::Completed
        );
    }
    #[test]
    fn stale_peer_is_unresolved() {
        let mut value = request();
        value.peers[0].fresh = false;
        assert_eq!(
            operate_federated_continual_context_compilation(&value)
                .unwrap()
                .disposition,
            FederatedContinualContextControlDisposition::Unresolved
        );
    }
    #[test]
    fn incomparable_peer_is_denied() {
        let mut value = request();
        value.peers[0].comparable = false;
        assert_eq!(
            operate_federated_continual_context_compilation(&value)
                .unwrap()
                .disposition,
            FederatedContinualContextControlDisposition::Denied
        );
    }
    #[test]
    fn quorum_failure_is_unresolved() {
        let mut value = request();
        value.peers[0].fresh = false;
        assert_eq!(
            operate_federated_continual_context_compilation(&value)
                .unwrap()
                .disposition,
            FederatedContinualContextControlDisposition::Unresolved
        );
    }
    #[test]
    fn policy_is_denied() {
        let mut value = request();
        value.policy_allow = false;
        assert_eq!(
            operate_federated_continual_context_compilation(&value)
                .unwrap()
                .disposition,
            FederatedContinualContextControlDisposition::Denied
        );
    }
    #[test]
    fn digest_is_stable() {
        let receipt = operate_federated_continual_context_compilation(&request()).unwrap();
        assert_eq!(receipt.digest().unwrap(), receipt.digest().unwrap());
    }
}
