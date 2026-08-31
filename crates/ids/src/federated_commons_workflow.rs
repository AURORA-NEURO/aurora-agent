//! Federated-commons workflow fabric (`AFA-ids-P31-F15`).
//!
//! Negotiates digest-only capability participation across institutions. It never exports raw
//! research data, executes a remote provider, or treats an incomplete quorum as success.

use crate::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-ids-P31-F15";
pub const CONTRACT_VERSION: &str = "ids-federated-commons-workflow-fabric/1.0";
pub const INPUT_SCHEMA: &str = "IdsFederatedCommonsRequest8@1";
pub const OUTPUT_SCHEMA: &str = "IdsFederatedCommonsReceipt10@1";
pub const CONTENT_TYPE: &str = "application/vnd.aurora.ids-federated-commons-receipt-10+json";
pub const PRECLINICAL_BOUNDARY: &str = "preclinical-research-only; no human-subject or clinical-source data; no diagnosis, treatment, triage, enrollment, or clinical decisions";
pub const MAX_PEERS: usize = 16_384;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CommonsEvidenceState {
    Proven,
    Supported,
    Unknown,
    Unmeasured,
    Contradicted,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IdsCommonsPeer7 {
    pub peer_id: String,
    pub institution_id: String,
    pub capability_id: String,
    pub semantic_profile: String,
    pub artifact_digest: ContentHash,
    pub evidence_state: CommonsEvidenceState,
    pub replay_identity: ContentHash,
    pub policy_allow: bool,
    pub federation_allow: bool,
    pub signed: bool,
    pub local: bool,
    pub aggregate_only: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IdsFederatedCommonsRequest8 {
    pub request_id: String,
    pub purpose: String,
    pub required_capability: String,
    pub semantic_profile: String,
    pub peers: Vec<IdsCommonsPeer7>,
    pub replay_identity: ContentHash,
    pub minimum_peer_quorum: u32,
    pub policy_allow: bool,
    pub protected_closure: bool,
    pub signed_approval: bool,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IdsFederatedCommonsReceipt10Artifact {
    pub artifact_id: String,
    pub content_type: String,
    pub content_hash: ContentHash,
    pub semantic_loss: Vec<String>,
    pub provenance_digests: Vec<ContentHash>,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IdsFederatedCommonsReceipt10 {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub purpose: String,
    pub required_capability: String,
    pub semantic_profile: String,
    pub disposition: String,
    pub peer_order: Vec<String>,
    pub qualified_peer_order: Vec<String>,
    pub unresolved_peer_order: Vec<String>,
    pub blocked_peer_order: Vec<String>,
    pub missing_peer_order: Vec<String>,
    pub contradiction_order: Vec<String>,
    pub evidence_order: Vec<String>,
    pub omission_order: Vec<String>,
    pub uncertainty_order: Vec<String>,
    pub negative_evidence_order: Vec<String>,
    pub minimum_peer_quorum: u32,
    pub qualified_peer_count: u32,
    pub effect_order: Vec<String>,
    pub replay_identity: ContentHash,
    pub commons_digest: ContentHash,
    pub artifact: IdsFederatedCommonsReceipt10Artifact,
    pub effect_receipts: Vec<String>,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub boundary: String,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum FederatedCommonsError {
    #[error("invalid IDS federated-commons request: {0}")]
    Invalid(String),
    #[error("IDS federated-commons report failed validation: {0}")]
    Report(String),
}

fn valid_digest(value: &ContentHash) -> bool {
    value.as_str().len() == 64 && value.as_str().bytes().all(|byte| byte.is_ascii_hexdigit())
}
fn ordered<T: Ord>(values: &[T]) -> bool {
    values.windows(2).all(|window| window[0] < window[1])
}

pub fn federated_commons_manifest() -> serde_json::Value {
    json!({"schema_version":"aurora-research-contract/1.0","capability_id":FEATURE_ID,"version":CONTRACT_VERSION,"owner_crate":"ids","consumers":["consortium researcher","federation operator","governance administrator"],"behavior":"negotiate digest-only peer participation with deterministic capability, semantic-profile, evidence, replay, quorum, policy, signature, and locality gates","value":"makes cross-institution research commons membership auditable without moving raw data or accepting incomplete quorum","input_schema":INPUT_SCHEMA,"output_schema":OUTPUT_SCHEMA,"effects":["exchange:federated-commons-digests","manage:local-capability"],"permissions":["read:local-peer-summaries","request:federated-commons-preview"],"autonomy_tier":"A2","boundary":PRECLINICAL_BOUNDARY})
}

impl IdsFederatedCommonsReceipt10 {
    pub fn validate(&self) -> Result<(), FederatedCommonsError> {
        if self.schema_version != "aurora-research-contract/1.0"
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.boundary != PRECLINICAL_BOUNDARY
            || self.artifact.boundary != PRECLINICAL_BOUNDARY
            || !self.raw_data_local
            || !self.aggregate_only
            || self.request_id.trim().is_empty()
            || self.purpose.trim().is_empty()
            || self.required_capability.trim().is_empty()
            || self.semantic_profile.trim().is_empty()
            || self.peer_order.is_empty()
            || self.effect_order.is_empty()
            || self.effect_receipts.is_empty()
            || self.minimum_peer_quorum == 0
            || !["qualified", "unresolved", "blocked"].contains(&self.disposition.as_str())
        {
            return Err(FederatedCommonsError::Report(
                "commons identity, peers, quorum, effects, locality, or disposition is incomplete"
                    .into(),
            ));
        }
        for values in [
            &self.peer_order,
            &self.qualified_peer_order,
            &self.unresolved_peer_order,
            &self.blocked_peer_order,
            &self.missing_peer_order,
            &self.contradiction_order,
            &self.evidence_order,
            &self.omission_order,
            &self.uncertainty_order,
            &self.negative_evidence_order,
            &self.effect_order,
            &self.effect_receipts,
        ] {
            if !ordered(values) {
                return Err(FederatedCommonsError::Report(
                    "commons ordering is not canonical".into(),
                ));
            }
        }
        let ids = BTreeSet::from_iter(self.peer_order.iter().cloned());
        let parts = self
            .qualified_peer_order
            .iter()
            .chain(&self.unresolved_peer_order)
            .chain(&self.blocked_peer_order)
            .cloned()
            .collect::<Vec<_>>();
        if ids.len() != self.peer_order.len()
            || parts.len() != ids.len()
            || BTreeSet::from_iter(parts) != ids
            || self.qualified_peer_count as usize != self.qualified_peer_order.len()
            || self.qualified_peer_count > self.peer_order.len() as u32
            || !valid_digest(&self.replay_identity)
            || !valid_digest(&self.commons_digest)
            || self.artifact.content_hash != self.commons_digest
            || self.artifact.content_type != CONTENT_TYPE
            || self
                .artifact
                .provenance_digests
                .iter()
                .any(|digest| !valid_digest(digest))
        {
            return Err(FederatedCommonsError::Report(
                "commons states, quorum, digests, or artifact metadata do not partition".into(),
            ));
        }
        if self.effect_receipts.iter().any(|effect| {
            !effect.starts_with("exchange:federated-commons-digests:")
                && !effect.starts_with("manage:local-capability:")
                && effect != "block:unsafe-release"
        }) {
            return Err(FederatedCommonsError::Report(
                "effect is outside governed commons gate".into(),
            ));
        }
        Ok(())
    }
    pub fn digest(&self) -> Result<ContentHash, FederatedCommonsError> {
        self.validate()?;
        let value = serde_json::to_value(self)
            .map_err(|error| FederatedCommonsError::Report(error.to_string()))?;
        ContentHash::of_value(&value)
            .map_err(|error| FederatedCommonsError::Report(error.to_string()))
    }
}

fn validate_request(request: &IdsFederatedCommonsRequest8) -> Result<(), FederatedCommonsError> {
    if request.request_id.trim().is_empty()
        || request.purpose.trim().is_empty()
        || request.required_capability.trim().is_empty()
        || request.semantic_profile.trim().is_empty()
        || request.peers.is_empty()
        || request.peers.len() > MAX_PEERS
        || request.minimum_peer_quorum == 0
        || !valid_digest(&request.replay_identity)
        || request.boundary != PRECLINICAL_BOUNDARY
        || !request.raw_data_local
        || !request.aggregate_only
    {
        return Err(FederatedCommonsError::Invalid(
            "commons identity, peer bound, quorum, replay, locality, or boundary is invalid".into(),
        ));
    }
    let mut ids = BTreeSet::new();
    for peer in &request.peers {
        if peer.peer_id.trim().is_empty()
            || !ids.insert(peer.peer_id.clone())
            || peer.institution_id.trim().is_empty()
            || peer.capability_id.trim().is_empty()
            || peer.semantic_profile.trim().is_empty()
            || !valid_digest(&peer.artifact_digest)
            || !valid_digest(&peer.replay_identity)
            || !peer.local
            || !peer.aggregate_only
        {
            return Err(FederatedCommonsError::Invalid(format!(
                "peer {} is invalid, duplicated, non-local, or not digest-bound",
                peer.peer_id
            )));
        }
    }
    Ok(())
}

pub fn preview_federated_commons(
    request: &IdsFederatedCommonsRequest8,
) -> Result<IdsFederatedCommonsReceipt10, FederatedCommonsError> {
    validate_request(request)?;
    let mut peers = request.peers.clone();
    peers.sort_by(|left, right| left.peer_id.cmp(&right.peer_id));
    let peer_order = peers
        .iter()
        .map(|peer| peer.peer_id.clone())
        .collect::<Vec<_>>();
    let mut qualified = BTreeSet::new();
    let mut unresolved = BTreeSet::new();
    let mut blocked = BTreeSet::new();
    let mut missing = BTreeSet::new();
    let mut contradiction = BTreeSet::new();
    let mut evidence = BTreeSet::new();
    let mut omissions = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    let mut negative = BTreeSet::new();
    let mut provenance = BTreeSet::new();
    for peer in &peers {
        provenance.insert(peer.artifact_digest.clone());
        if peer.capability_id != request.required_capability {
            unresolved.insert(peer.peer_id.clone());
            missing.insert(format!("{}:capability", peer.peer_id));
        } else if peer.semantic_profile != request.semantic_profile {
            unresolved.insert(peer.peer_id.clone());
            uncertainty.insert(format!("{}:semantic-profile", peer.peer_id));
        } else if peer.replay_identity != request.replay_identity {
            unresolved.insert(peer.peer_id.clone());
            uncertainty.insert(format!("{}:replay-identity", peer.peer_id));
        } else if !peer.policy_allow || !peer.federation_allow || !peer.signed {
            blocked.insert(peer.peer_id.clone());
            omissions.insert(format!("{}:peer-governance", peer.peer_id));
        } else if peer.evidence_state == CommonsEvidenceState::Contradicted {
            blocked.insert(peer.peer_id.clone());
            contradiction.insert(peer.peer_id.clone());
            negative.insert(format!("{}:contradicted", peer.peer_id));
        } else if !matches!(
            peer.evidence_state,
            CommonsEvidenceState::Proven | CommonsEvidenceState::Supported
        ) {
            unresolved.insert(peer.peer_id.clone());
            evidence.insert(peer.peer_id.clone());
            uncertainty.insert(format!("{}:evidence-state", peer.peer_id));
        } else {
            qualified.insert(peer.peer_id.clone());
        }
    }
    let global = !request.policy_allow
        || !request.protected_closure
        || !request.signed_approval
        || !request.raw_data_local
        || !request.aggregate_only;
    if global {
        blocked.extend(peer_order.iter().cloned());
        qualified.clear();
        unresolved.clear();
        omissions.insert("request:governance-or-locality-denied".into());
    }
    let qualified_peer_order = qualified.into_iter().collect::<Vec<_>>();
    let unresolved_peer_order = unresolved.into_iter().collect::<Vec<_>>();
    let blocked_peer_order = blocked.into_iter().collect::<Vec<_>>();
    let disposition =
        if global || qualified_peer_order.len() < (request.minimum_peer_quorum as usize) {
            if global || qualified_peer_order.is_empty() {
                "blocked"
            } else {
                "unresolved"
            }
        } else if !unresolved_peer_order.is_empty() || !blocked_peer_order.is_empty() {
            "unresolved"
        } else {
            "qualified"
        };
    if qualified_peer_order.len() < (request.minimum_peer_quorum as usize) {
        omissions.insert("request:peer-quorum-not-met".into());
    }
    if disposition != "qualified" {
        omissions.insert("request:federated-commons-not-closed".into());
    }
    let effect_order = if disposition == "qualified" {
        vec![
            "exchange:federated-commons-digests".to_string(),
            "manage:local-capability".to_string(),
        ]
    } else {
        vec!["block:unsafe-release".to_string()]
    };
    let mut effect_order = effect_order;
    effect_order.sort();
    let payload = json!({"schema_version":"aurora-research-contract/1.0","contract_version":CONTRACT_VERSION,"feature_id":FEATURE_ID,"request_id":request.request_id,"purpose":request.purpose,"required_capability":request.required_capability,"semantic_profile":request.semantic_profile,"disposition":disposition,"peer_order":peer_order,"qualified_peer_order":qualified_peer_order,"unresolved_peer_order":unresolved_peer_order,"blocked_peer_order":blocked_peer_order,"missing_peer_order":missing.into_iter().collect::<Vec<_>>(),"contradiction_order":contradiction.into_iter().collect::<Vec<_>>(),"evidence_order":evidence.into_iter().collect::<Vec<_>>(),"omission_order":omissions.into_iter().collect::<Vec<_>>(),"uncertainty_order":uncertainty.into_iter().collect::<Vec<_>>(),"negative_evidence_order":negative.into_iter().collect::<Vec<_>>(),"minimum_peer_quorum":request.minimum_peer_quorum,"qualified_peer_count":qualified_peer_order.len(),"effect_order":effect_order,"replay_identity":request.replay_identity,"raw_data_local":true,"aggregate_only":true,"boundary":PRECLINICAL_BOUNDARY});
    let digest = ContentHash::of_value(&payload)
        .map_err(|error| FederatedCommonsError::Report(error.to_string()))?;
    let report = IdsFederatedCommonsReceipt10 {
        schema_version: "aurora-research-contract/1.0".into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        request_id: request.request_id.clone(),
        purpose: request.purpose.clone(),
        required_capability: request.required_capability.clone(),
        semantic_profile: request.semantic_profile.clone(),
        disposition: disposition.into(),
        peer_order: serde_json::from_value(payload["peer_order"].clone()).unwrap(),
        qualified_peer_order: serde_json::from_value(payload["qualified_peer_order"].clone())
            .unwrap(),
        unresolved_peer_order: serde_json::from_value(payload["unresolved_peer_order"].clone())
            .unwrap(),
        blocked_peer_order: serde_json::from_value(payload["blocked_peer_order"].clone()).unwrap(),
        missing_peer_order: serde_json::from_value(payload["missing_peer_order"].clone()).unwrap(),
        contradiction_order: serde_json::from_value(payload["contradiction_order"].clone())
            .unwrap(),
        evidence_order: serde_json::from_value(payload["evidence_order"].clone()).unwrap(),
        omission_order: serde_json::from_value(payload["omission_order"].clone()).unwrap(),
        uncertainty_order: serde_json::from_value(payload["uncertainty_order"].clone()).unwrap(),
        negative_evidence_order: serde_json::from_value(payload["negative_evidence_order"].clone())
            .unwrap(),
        minimum_peer_quorum: request.minimum_peer_quorum,
        qualified_peer_count: qualified_peer_order.len() as u32,
        effect_order: serde_json::from_value(payload["effect_order"].clone()).unwrap(),
        replay_identity: request.replay_identity.clone(),
        commons_digest: digest.clone(),
        artifact: IdsFederatedCommonsReceipt10Artifact {
            artifact_id: format!("ids-federated-commons-receipt-10:{}", request.request_id),
            content_type: CONTENT_TYPE.into(),
            content_hash: digest,
            semantic_loss: serde_json::from_value(payload["omission_order"].clone()).unwrap(),
            provenance_digests: provenance.into_iter().collect(),
            boundary: PRECLINICAL_BOUNDARY.into(),
        },
        effect_receipts: effect_order
            .iter()
            .map(|effect| {
                if effect == "block:unsafe-release" {
                    effect.clone()
                } else {
                    format!("{effect}:{}", request.request_id)
                }
            })
            .collect(),
        raw_data_local: true,
        aggregate_only: true,
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    report.validate()?;
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn h(value: &str) -> ContentHash {
        ContentHash::of_bytes(value.as_bytes())
    }
    fn peer(id: &str) -> IdsCommonsPeer7 {
        IdsCommonsPeer7 {
            peer_id: id.into(),
            institution_id: format!("inst:{id}"),
            capability_id: "ids.capacity".into(),
            semantic_profile: "ids-v1".into(),
            artifact_digest: h(id),
            evidence_state: CommonsEvidenceState::Supported,
            replay_identity: h("replay"),
            policy_allow: true,
            federation_allow: true,
            signed: true,
            local: true,
            aggregate_only: true,
        }
    }
    fn request() -> IdsFederatedCommonsRequest8 {
        IdsFederatedCommonsRequest8 {
            request_id: "request:commons".into(),
            purpose: "join".into(),
            required_capability: "ids.capacity".into(),
            semantic_profile: "ids-v1".into(),
            peers: vec![peer("peer:b"), peer("peer:a")],
            replay_identity: h("replay"),
            minimum_peer_quorum: 2,
            policy_allow: true,
            protected_closure: true,
            signed_approval: true,
            raw_data_local: true,
            aggregate_only: true,
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }
    #[test]
    fn manifest_is_a2() {
        assert_eq!(federated_commons_manifest()["autonomy_tier"], "A2");
    }
    #[test]
    fn quorum_is_qualified() {
        assert_eq!(
            preview_federated_commons(&request()).unwrap().disposition,
            "qualified"
        );
    }
    #[test]
    fn missing_capability_is_unresolved() {
        let mut q = request();
        q.peers[0].capability_id = "other".into();
        assert_eq!(
            preview_federated_commons(&q).unwrap().disposition,
            "unresolved"
        );
    }
    #[test]
    fn contradiction_is_unresolved() {
        let mut q = request();
        q.peers[0].evidence_state = CommonsEvidenceState::Contradicted;
        assert_eq!(
            preview_federated_commons(&q).unwrap().disposition,
            "unresolved"
        );
    }
    #[test]
    fn policy_denial_blocks() {
        let mut q = request();
        q.policy_allow = false;
        assert_eq!(
            preview_federated_commons(&q).unwrap().disposition,
            "blocked"
        );
    }
    #[test]
    fn digest_is_deterministic() {
        let a = preview_federated_commons(&request()).unwrap();
        let b = preview_federated_commons(&request()).unwrap();
        assert_eq!(a.digest().unwrap(), b.digest().unwrap());
    }
}
