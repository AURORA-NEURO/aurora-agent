//! Federated continual knowledge-representation inference engine.
//! Atlas feature: `AFA-brain-P04-F04`.

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

pub const FEATURE_ID: &str = "AFA-brain-P04-F04";
pub const CONTRACT_VERSION: &str =
    "brain-federated-continual-knowledge-representation-inference-engine/1.0";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederatedKnowledgePeer {
    pub peer_id: String,
    pub institution_id: String,
    pub claims_digest: ContentHash,
    pub world_digest: ContentHash,
    pub evidence_digest: Option<ContentHash>,
    pub provenance_digest: Option<ContentHash>,
    pub replay_identity: ContentHash,
    pub semantic_profile: String,
    pub fresh: bool,
    pub comparable: bool,
    pub permitted_summary: bool,
    pub signed_approval: bool,
    pub policy_allow: bool,
    pub state: EvidenceState,
    pub raw_data_local: bool,
    pub boundary: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederatedKnowledgeRequest {
    pub request_id: String,
    pub federation_id: String,
    pub peers: Vec<FederatedKnowledgePeer>,
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
pub enum FederatedKnowledgeDisposition {
    Completed,
    Partial,
    Unresolved,
    Denied,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederatedKnowledgeReceipt {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub federation_id: String,
    pub disposition: FederatedKnowledgeDisposition,
    pub candidate_order: Vec<String>,
    pub qualified_order: Vec<String>,
    pub unresolved_order: Vec<String>,
    pub denied_order: Vec<String>,
    pub exchange_order: Vec<ContentHash>,
    pub semantic_profile_order: Vec<String>,
    pub freshness_order: Vec<String>,
    pub quorum_required: u16,
    pub quorum_met: bool,
    pub run_digest: ContentHash,
    pub evidence_digest: ContentHash,
    pub provenance_digest: ContentHash,
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
pub enum FederatedKnowledgeError {
    #[error("invalid federated knowledge request: {0}")]
    Invalid(String),
    #[error("federated knowledge artifact failed: {0}")]
    Artifact(String),
}

impl FederatedKnowledgeReceipt {
    pub fn validate(&self) -> Result<(), FederatedKnowledgeError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.boundary != PRECLINICAL_BOUNDARY
            || !self.raw_data_local
            || self.request_id.trim().is_empty()
            || self.federation_id.trim().is_empty()
            || self.candidate_order.is_empty()
            || self.quorum_required == 0
            || self.effect_receipts.is_empty()
        {
            return Err(FederatedKnowledgeError::Invalid(
                "federated knowledge identity, quorum, locality, or effects are incomplete".into(),
            ));
        }
        for values in [
            &self.candidate_order,
            &self.qualified_order,
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
                return Err(FederatedKnowledgeError::Invalid(
                    "federated knowledge ordering is not canonical".into(),
                ));
            }
        }
        if self
            .exchange_order
            .windows(2)
            .any(|pair| pair[0] >= pair[1])
        {
            return Err(FederatedKnowledgeError::Invalid(
                "federated knowledge exchange ordering is not canonical".into(),
            ));
        }
        let classified = self
            .qualified_order
            .iter()
            .chain(self.unresolved_order.iter())
            .chain(self.denied_order.iter())
            .cloned()
            .collect::<BTreeSet<_>>();
        if classified.len() != self.candidate_order.len()
            || classified
                .iter()
                .any(|peer| !self.candidate_order.contains(peer))
        {
            return Err(FederatedKnowledgeError::Invalid(
                "federated knowledge states do not partition peers".into(),
            ));
        }
        let quorum_observed = self.qualified_order.len() >= usize::from(self.quorum_required);
        if self.quorum_met != quorum_observed {
            return Err(FederatedKnowledgeError::Invalid(
                "federated knowledge quorum witness is inconsistent".into(),
            ));
        }
        if self.exchange_order.len() != self.qualified_order.len() {
            return Err(FederatedKnowledgeError::Invalid(
                "federated knowledge exchange does not match qualified peers".into(),
            ));
        }
        for digest in self.exchange_order.iter().chain([
            &self.run_digest,
            &self.evidence_digest,
            &self.provenance_digest,
            &self.replay_identity,
        ]) {
            if digest.as_str().len() != 64 {
                return Err(FederatedKnowledgeError::Invalid(
                    "federated knowledge digest is invalid".into(),
                ));
            }
        }
        let expected_effect = if self.disposition == FederatedKnowledgeDisposition::Completed {
            format!("read:local-federated-knowledge:{}", self.request_id)
        } else {
            "block:unsafe-release".into()
        };
        if self.effect_receipts != [expected_effect] {
            return Err(FederatedKnowledgeError::Invalid(
                "federated knowledge effect does not match disposition".into(),
            ));
        }
        let expected_evidence = ContentHash::of_value(&json!({
            "candidate_order": self.candidate_order,
            "qualified_order": self.qualified_order,
            "unresolved_order": self.unresolved_order,
            "denied_order": self.denied_order,
        }))
        .map_err(|error| FederatedKnowledgeError::Artifact(error.to_string()))?;
        if self.evidence_digest != expected_evidence {
            return Err(FederatedKnowledgeError::Invalid(
                "federated knowledge evidence digest is not bound to peer state".into(),
            ));
        }
        let expected_provenance = ContentHash::of_value(&json!({
            "federation_id": self.federation_id,
            "exchange_order": self.exchange_order,
            "replay_identity": self.replay_identity,
        }))
        .map_err(|error| FederatedKnowledgeError::Artifact(error.to_string()))?;
        if self.provenance_digest != expected_provenance {
            return Err(FederatedKnowledgeError::Invalid(
                "federated knowledge provenance digest is not bound to exchange".into(),
            ));
        }
        let expected_run = ContentHash::of_value(&json!({
            "feature_id": FEATURE_ID,
            "request_id": self.request_id,
            "disposition": self.disposition,
            "qualified_order": self.qualified_order,
            "unresolved_order": self.unresolved_order,
            "denied_order": self.denied_order,
            "quorum_met": self.quorum_met,
            "evidence_digest": self.evidence_digest,
            "provenance_digest": self.provenance_digest,
            "replay_identity": self.replay_identity,
        }))
        .map_err(|error| FederatedKnowledgeError::Artifact(error.to_string()))?;
        if self.run_digest != expected_run {
            return Err(FederatedKnowledgeError::Invalid(
                "federated knowledge run digest is not bound to execution state".into(),
            ));
        }
        let expected_artifact_id = format!(
            "brain-federated-continual-knowledge-representation:{}",
            self.request_id
        );
        if self.artifact.artifact_id != expected_artifact_id
            || self.artifact.content_type
                != "application/vnd.aurora.federated-continual-knowledge-world+json"
            || !self.artifact.semantic_loss.is_empty()
            || !self.artifact.provenance.is_empty()
        {
            return Err(FederatedKnowledgeError::Invalid(
                "federated knowledge artifact identity or provenance is inconsistent".into(),
            ));
        }
        self.artifact
            .validate_metadata()
            .map_err(|error| FederatedKnowledgeError::Artifact(error.to_string()))?;
        self.artifact
            .verify_payload(&receipt_payload(self))
            .map_err(|error| FederatedKnowledgeError::Artifact(error.to_string()))
    }
    pub fn digest(&self) -> Result<ContentHash, FederatedKnowledgeError> {
        self.validate()?;
        ContentHash::of_value(
            &serde_json::to_value(self)
                .map_err(|error| FederatedKnowledgeError::Artifact(error.to_string()))?,
        )
        .map_err(|error| FederatedKnowledgeError::Artifact(error.to_string()))
    }
}

pub fn federated_continual_knowledge_representation_inference_engine_manifest() -> CapabilityManifest
{
    CapabilityManifest { schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(), capability_id: FEATURE_ID.into(), version: CONTRACT_VERSION.into(), owner_crate: "brain".into(), consumers: ["laboratory automation engineer".into(), "federation administrator".into()].into(), behavior: "qualifies fresh, comparable, signed peer knowledge-world summaries under quorum and purpose-bound federation gates".into(), value: "prevents stale, semantically incompatible, or unauthorized peer knowledge from becoming a continual federated result".into(), inputs: vec![TypedPort { name: "federated_knowledge_request".into(), schema: "ScopedResearchClaims4@1".into(), required: true }], outputs: vec![TypedPort { name: "federated_knowledge_receipt".into(), schema: "TypedKnowledgeWorld1@1".into(), required: true }], effects: [Effect::ReadLocalData, Effect::WriteLocalArtifact].into(), permissions: ["read:local-research-artifacts".into()].into(), determinism: Determinism::ByteStable, evidence: vec![EvidenceReference { source_id: "json-schema".into(), state: EvidenceState::Supported, locator: Some("https://json-schema.org/specification".into()) }], authority_requirements: Vec::new(), autonomy_tier: AutonomyTier::A1, surfaces: [ResearchSurface::Ui, ResearchSurface::Api, ResearchSurface::Sdk, ResearchSurface::Cli, ResearchSurface::McpTool, ResearchSurface::Operator].into(), boundary: PRECLINICAL_BOUNDARY.into() }
}

fn receipt_payload(receipt: &FederatedKnowledgeReceipt) -> serde_json::Value {
    json!({
        "schema_version": receipt.schema_version,
        "contract_version": receipt.contract_version,
        "feature_id": receipt.feature_id,
        "request_id": receipt.request_id,
        "federation_id": receipt.federation_id,
        "disposition": receipt.disposition,
        "candidate_order": receipt.candidate_order,
        "qualified_order": receipt.qualified_order,
        "unresolved_order": receipt.unresolved_order,
        "denied_order": receipt.denied_order,
        "exchange_order": receipt.exchange_order,
        "semantic_profile_order": receipt.semantic_profile_order,
        "freshness_order": receipt.freshness_order,
        "quorum_required": receipt.quorum_required,
        "quorum_met": receipt.quorum_met,
        "run_digest": receipt.run_digest,
        "evidence_digest": receipt.evidence_digest,
        "provenance_digest": receipt.provenance_digest,
        "replay_identity": receipt.replay_identity,
        "boundary": receipt.boundary,
    })
}

pub fn infer_federated_continual_knowledge_representation(
    request: &FederatedKnowledgeRequest,
) -> Result<FederatedKnowledgeReceipt, FederatedKnowledgeError> {
    if request.request_id.trim().is_empty()
        || request.federation_id.trim().is_empty()
        || request.peers.is_empty()
        || request.min_quorum == 0
        || usize::from(request.min_quorum) > request.peers.len()
        || request.replay_identity.as_str().len() != 64
        || !request.raw_data_local
        || request.boundary != PRECLINICAL_BOUNDARY
    {
        return Err(FederatedKnowledgeError::Invalid("federated knowledge identity, peer set, quorum, replay, locality, or boundary is invalid".into()));
    }
    let mut peers = request.peers.clone();
    peers.sort_by(|a, b| a.peer_id.cmp(&b.peer_id));
    let candidate = peers.iter().map(|p| p.peer_id.clone()).collect::<Vec<_>>();
    if candidate.windows(2).any(|p| p[0] == p[1]) || candidate.iter().any(|v| v.trim().is_empty()) {
        return Err(FederatedKnowledgeError::Invalid(
            "federated peer identifiers must be unique and non-empty".into(),
        ));
    }
    let map = peers
        .iter()
        .map(|p| (p.peer_id.clone(), p))
        .collect::<BTreeMap<_, _>>();
    let mut qualified = BTreeSet::new();
    let mut unresolved = BTreeSet::new();
    let mut denied = BTreeSet::new();
    let mut exchange = Vec::new();
    let mut profiles = BTreeSet::new();
    let mut fresh = BTreeSet::new();
    let mut witnesses = BTreeSet::from([
        "gate:typed-federated-knowledge".to_string(),
        "gate:freshness".to_string(),
        "gate:semantic-comparability".to_string(),
        "gate:quorum".to_string(),
        "gate:replay-identity".to_string(),
        "gate:purpose-bound-summary".to_string(),
        "gate:locality".to_string(),
    ]);
    let mut counter = BTreeSet::new();
    let mut omissions = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    let mut negative = BTreeSet::new();
    let global = request.policy_allow
        && request.protected_closure
        && request.raw_data_local
        && request.signed_approval;
    for id in &candidate {
        let p = map[id];
        if !global
            || !p.policy_allow
            || !p.signed_approval
            || !p.permitted_summary
            || !p.raw_data_local
            || p.boundary != PRECLINICAL_BOUNDARY
        {
            denied.insert(id.clone());
            counter.insert(format!(
                "counterexample:{}:policy-approval-purpose-locality",
                id
            ));
        } else if !p.fresh {
            unresolved.insert(id.clone());
            omissions.insert(format!("peer:{}:stale-knowledge", id));
        } else if !p.comparable || p.semantic_profile.trim().is_empty() {
            denied.insert(id.clone());
            counter.insert(format!("counterexample:{}:semantic-profile-mismatch", id));
        } else if p.replay_identity != request.replay_identity {
            unresolved.insert(id.clone());
            uncertainty.insert(format!("peer:{}:replay-mismatch", id));
        } else if p.evidence_digest.is_none() || p.provenance_digest.is_none() {
            unresolved.insert(id.clone());
            omissions.insert(format!("peer:{}:evidence-or-provenance-missing", id));
        } else if matches!(p.state, EvidenceState::Unknown | EvidenceState::Speculative) {
            unresolved.insert(id.clone());
            uncertainty.insert(format!("peer:{}:unknown-not-asserted", id));
        } else if matches!(p.state, EvidenceState::Contradicted) {
            denied.insert(id.clone());
            negative.insert(format!("peer:{}:contradicted", id));
        } else {
            qualified.insert(id.clone());
            profiles.insert(p.semantic_profile.clone());
            fresh.insert(id.clone());
            exchange.push(ContentHash::of_value(&json!({"peer_id":p.peer_id,"institution_id":p.institution_id,"claims_digest":p.claims_digest,"world_digest":p.world_digest,"evidence_digest":p.evidence_digest,"provenance_digest":p.provenance_digest,"semantic_profile":p.semantic_profile,"replay_identity":p.replay_identity})).map_err(|e|FederatedKnowledgeError::Artifact(e.to_string()))?);
        }
    }
    let quorum_met = qualified.len() >= usize::from(request.min_quorum);
    if !quorum_met {
        omissions.insert("federation:quorum-incomplete".into());
        uncertainty.insert(format!(
            "quorum:required-{}:observed-{}",
            request.min_quorum,
            qualified.len()
        ));
    }
    if !request.policy_allow {
        counter.insert("counterexample:policy-denied".into());
        omissions.insert("control:policy-denied".into());
    }
    if !request.protected_closure {
        counter.insert("counterexample:protected-closure-incomplete".into());
        omissions.insert("control:protected-closure-incomplete".into());
    }
    if !unresolved.is_empty() {
        witnesses.insert("gate:partial-peer-results-retained".into());
    }
    exchange.sort_by(|a, b| a.as_str().cmp(b.as_str()));
    let disposition = if !global || !denied.is_empty() {
        FederatedKnowledgeDisposition::Denied
    } else if !quorum_met {
        FederatedKnowledgeDisposition::Unresolved
    } else if !unresolved.is_empty() {
        FederatedKnowledgeDisposition::Partial
    } else {
        FederatedKnowledgeDisposition::Completed
    };
    let evidence=ContentHash::of_value(&json!({"candidate_order":candidate,"qualified_order":qualified,"unresolved_order":unresolved,"denied_order":denied})).map_err(|e|FederatedKnowledgeError::Artifact(e.to_string()))?;
    let provenance=ContentHash::of_value(&json!({"federation_id":request.federation_id,"exchange_order":exchange,"replay_identity":request.replay_identity})).map_err(|e|FederatedKnowledgeError::Artifact(e.to_string()))?;
    let run=ContentHash::of_value(&json!({"feature_id":FEATURE_ID,"request_id":request.request_id,"disposition":disposition,"qualified_order":qualified,"unresolved_order":unresolved,"denied_order":denied,"quorum_met":quorum_met,"evidence_digest":evidence,"provenance_digest":provenance,"replay_identity":request.replay_identity})).map_err(|e|FederatedKnowledgeError::Artifact(e.to_string()))?;
    let payload = json!({"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"contract_version":CONTRACT_VERSION,"feature_id":FEATURE_ID,"request_id":request.request_id,"federation_id":request.federation_id,"disposition":disposition,"candidate_order":candidate,"qualified_order":qualified,"unresolved_order":unresolved,"denied_order":denied,"exchange_order":exchange,"semantic_profile_order":profiles,"freshness_order":fresh,"quorum_required":request.min_quorum,"quorum_met":quorum_met,"run_digest":run,"evidence_digest":evidence,"provenance_digest":provenance,"replay_identity":request.replay_identity,"boundary":PRECLINICAL_BOUNDARY});
    let artifact = TypedResearchArtifact::from_payload(
        format!(
            "brain-federated-continual-knowledge-representation:{}",
            request.request_id
        ),
        "application/vnd.aurora.federated-continual-knowledge-world+json",
        &payload,
        Vec::new(),
        Vec::new(),
    )
    .map_err(|e| FederatedKnowledgeError::Artifact(e.to_string()))?;
    let receipt = FederatedKnowledgeReceipt {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        request_id: request.request_id.clone(),
        federation_id: request.federation_id.clone(),
        disposition,
        candidate_order: candidate,
        qualified_order: qualified.into_iter().collect(),
        unresolved_order: unresolved.into_iter().collect(),
        denied_order: denied.into_iter().collect(),
        exchange_order: exchange,
        semantic_profile_order: profiles.into_iter().collect(),
        freshness_order: fresh.into_iter().collect(),
        quorum_required: request.min_quorum,
        quorum_met,
        run_digest: run,
        evidence_digest: evidence,
        provenance_digest: provenance,
        replay_identity: request.replay_identity.clone(),
        witness_order: witnesses.into_iter().collect(),
        counterexample_order: counter.into_iter().collect(),
        omissions: omissions.into_iter().collect(),
        uncertainty: uncertainty.into_iter().collect(),
        negative_evidence: negative.into_iter().collect(),
        effect_receipts: if matches!(disposition, FederatedKnowledgeDisposition::Completed) {
            vec![format!(
                "read:local-federated-knowledge:{}",
                request.request_id
            )]
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
    fn h(v: &str) -> ContentHash {
        ContentHash::of_bytes(v.as_bytes())
    }
    fn req() -> FederatedKnowledgeRequest {
        let r = h("federated-knowledge");
        let peer = |id: &str| FederatedKnowledgePeer {
            peer_id: id.into(),
            institution_id: format!("institution:{}", id),
            claims_digest: r.clone(),
            world_digest: r.clone(),
            evidence_digest: Some(r.clone()),
            provenance_digest: Some(r.clone()),
            replay_identity: r.clone(),
            semantic_profile: "profile:v1".into(),
            fresh: true,
            comparable: true,
            permitted_summary: true,
            signed_approval: true,
            policy_allow: true,
            state: EvidenceState::Supported,
            raw_data_local: true,
            boundary: PRECLINICAL_BOUNDARY.into(),
        };
        FederatedKnowledgeRequest {
            request_id: "request:federated-knowledge".into(),
            federation_id: "federation:one".into(),
            peers: vec![peer("peer:a"), peer("peer:b")],
            min_quorum: 2,
            replay_identity: r,
            policy_allow: true,
            protected_closure: true,
            raw_data_local: true,
            signed_approval: true,
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }
    #[test]
    fn manifest_is_a1() {
        assert_eq!(
            federated_continual_knowledge_representation_inference_engine_manifest().autonomy_tier,
            AutonomyTier::A1
        )
    }
    #[test]
    fn quorum_is_completed() {
        assert_eq!(
            infer_federated_continual_knowledge_representation(&req())
                .unwrap()
                .disposition,
            FederatedKnowledgeDisposition::Completed
        )
    }
    #[test]
    fn stale_is_unresolved() {
        let mut v = req();
        v.peers[0].fresh = false;
        assert_eq!(
            infer_federated_continual_knowledge_representation(&v)
                .unwrap()
                .disposition,
            FederatedKnowledgeDisposition::Unresolved
        )
    }
    #[test]
    fn profile_mismatch_is_denied() {
        let mut v = req();
        v.peers[0].comparable = false;
        assert_eq!(
            infer_federated_continual_knowledge_representation(&v)
                .unwrap()
                .disposition,
            FederatedKnowledgeDisposition::Denied
        )
    }
    #[test]
    fn quorum_failure_is_unresolved() {
        let mut v = req();
        v.peers[0].fresh = false;
        assert_eq!(
            infer_federated_continual_knowledge_representation(&v)
                .unwrap()
                .disposition,
            FederatedKnowledgeDisposition::Unresolved
        )
    }
    #[test]
    fn policy_is_denied() {
        let mut v = req();
        v.policy_allow = false;
        assert_eq!(
            infer_federated_continual_knowledge_representation(&v)
                .unwrap()
                .disposition,
            FederatedKnowledgeDisposition::Denied
        )
    }
    #[test]
    fn digest_is_stable() {
        let r = infer_federated_continual_knowledge_representation(&req()).unwrap();
        assert_eq!(r.digest().unwrap(), r.digest().unwrap())
    }
}
