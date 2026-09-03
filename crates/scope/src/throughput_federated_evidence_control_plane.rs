//! Prospective high-throughput federated evidence-surveillance control plane
//! (`AFA-scope-P01-F31`).
//!
//! The control plane admits bounded, caller-supplied evidence summaries and peer attestations;
//! it does not retrieve sources, move raw payloads, execute tools, or make clinical decisions.

use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-scope-P01-F31";
pub const CONTRACT_VERSION: &str =
    "scope-prospective-high-throughput-evidence-surveillance-federated-control-plane/1.0";
pub const INPUT_SCHEMA: &str = "EvidenceFeed3@1";
pub const OUTPUT_SCHEMA: &str = "FederatedEvidenceControlReceipt9@1";
pub const CONTENT_TYPE: &str =
    "application/vnd.aurora.scope-federated-evidence-control-receipt-9+json";
pub const SCHEMA_VERSION: &str = "aurora-research-contract/1.0";
pub const PRECLINICAL_BOUNDARY: &str = "preclinical-research-only; no human-subject or clinical-source data; no diagnosis, treatment, triage, enrollment, or clinical decisions";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ThroughputEvidence4 {
    pub evidence_id: String,
    pub source_id: String,
    pub sequence: u64,
    pub relevance_milli: u16,
    pub freshness_milli: u16,
    pub semantic_profile: String,
    pub content_digest: ContentHash,
    pub provenance_digest: ContentHash,
    pub replay_identity: ContentHash,
    pub evidence_state: String,
    pub available: bool,
    pub permitted: bool,
    pub aggregate_only: bool,
    pub raw_data_local: bool,
    pub negative_result: bool,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PeerEvidence4 {
    pub peer_id: String,
    pub semantic_profile: String,
    pub checkpoint_seq: u64,
    pub attestation_digest: ContentHash,
    pub signed: bool,
    pub aggregate_only: bool,
    pub raw_data_local: bool,
    pub available: bool,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceControlRequest6 {
    pub request_id: String,
    pub batch_id: String,
    pub partition: String,
    pub scope: String,
    pub semantic_profile: String,
    pub checkpoint_seq: u64,
    pub max_items: usize,
    pub max_budget_units: u64,
    pub min_peer_quorum: usize,
    pub replay_identity: ContentHash,
    pub policy_allow: bool,
    pub protected_closure: bool,
    pub federation_approved: bool,
    pub signed_approval: bool,
    pub aggregate_only: bool,
    pub raw_data_local: bool,
    pub boundary: String,
    pub observations: Vec<ThroughputEvidence4>,
    pub peers: Vec<PeerEvidence4>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederatedEvidenceControlArtifact9 {
    pub schema_version: String,
    pub artifact_id: String,
    pub content_type: String,
    pub content_hash: ContentHash,
    pub semantic_loss: Vec<String>,
    pub provenance_digests: Vec<ContentHash>,
    pub boundary: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederatedEvidenceControlReceipt9 {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub batch_id: String,
    pub partition: String,
    pub scope: String,
    pub semantic_profile: String,
    pub checkpoint_seq: u64,
    pub disposition: String,
    pub candidate_order: Vec<String>,
    pub ranked_order: Vec<String>,
    pub qualified_order: Vec<String>,
    pub unresolved_order: Vec<String>,
    pub blocked_order: Vec<String>,
    pub overflow_order: Vec<String>,
    pub peer_order: Vec<String>,
    pub qualified_peer_order: Vec<String>,
    pub missing_peer_order: Vec<String>,
    pub omission_order: Vec<String>,
    pub uncertainty_order: Vec<String>,
    pub negative_evidence_order: Vec<String>,
    pub replay_identity: ContentHash,
    pub checkpoint_digest: ContentHash,
    pub control_digest: ContentHash,
    pub artifact: FederatedEvidenceControlArtifact9,
    pub effect_receipts: Vec<String>,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub boundary: String,
}
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum EvidenceControlError {
    #[error("invalid federated evidence control request or receipt: {0}")]
    Invalid(String),
    #[error("federated evidence control artifact failed: {0}")]
    Artifact(String),
}
fn canonical(v: &[String]) -> bool {
    v.windows(2).all(|w| w[0] < w[1])
}
fn digest(v: &ContentHash) -> bool {
    v.as_str().len() == 64
}
impl FederatedEvidenceControlReceipt9 {
    pub fn validate(&self) -> Result<(), EvidenceControlError> {
        if self.schema_version != SCHEMA_VERSION
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.boundary != PRECLINICAL_BOUNDARY
            || self.artifact.boundary != PRECLINICAL_BOUNDARY
            || !self.raw_data_local
            || !self.aggregate_only
            || self.request_id.trim().is_empty()
            || self.batch_id.trim().is_empty()
            || self.partition.trim().is_empty()
            || self.scope.trim().is_empty()
            || self.semantic_profile.trim().is_empty()
            || self.checkpoint_seq == 0
            || self.candidate_order.is_empty()
            || self.ranked_order.is_empty()
            || self.peer_order.is_empty()
            || self.effect_receipts.is_empty()
        {
            return Err(EvidenceControlError::Invalid(
                "control identity, bounds, candidates, peers, locality, or effects are incomplete"
                    .into(),
            ));
        }
        for v in [
            &self.candidate_order,
            &self.ranked_order,
            &self.qualified_order,
            &self.unresolved_order,
            &self.blocked_order,
            &self.overflow_order,
            &self.peer_order,
            &self.qualified_peer_order,
            &self.missing_peer_order,
            &self.omission_order,
            &self.uncertainty_order,
            &self.negative_evidence_order,
            &self.effect_receipts,
        ] {
            if !canonical(v) {
                return Err(EvidenceControlError::Invalid(
                    "control ordering is not canonical".into(),
                ));
            }
        }
        let ids = self
            .candidate_order
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>();
        let parts = self
            .qualified_order
            .iter()
            .chain(&self.unresolved_order)
            .chain(&self.blocked_order)
            .chain(&self.overflow_order)
            .cloned()
            .collect::<Vec<_>>();
        if ids.len() != self.candidate_order.len()
            || self.ranked_order.iter().cloned().collect::<BTreeSet<_>>() != ids
            || parts.len() != ids.len()
            || parts.iter().cloned().collect::<BTreeSet<_>>() != ids
        {
            return Err(EvidenceControlError::Invalid(
                "candidate states or ranking do not partition".into(),
            ));
        }
        let peers = self.peer_order.iter().cloned().collect::<BTreeSet<_>>();
        let pp = self
            .qualified_peer_order
            .iter()
            .chain(&self.missing_peer_order)
            .cloned()
            .collect::<Vec<_>>();
        if peers.len() != self.peer_order.len()
            || pp.len() != peers.len()
            || pp.iter().cloned().collect::<BTreeSet<_>>() != peers
        {
            return Err(EvidenceControlError::Invalid(
                "peer states do not partition".into(),
            ));
        }
        if !digest(&self.replay_identity)
            || !digest(&self.checkpoint_digest)
            || !digest(&self.control_digest)
            || self.artifact.content_hash != self.control_digest
            || self.artifact.content_type != CONTENT_TYPE
            || !self.artifact.provenance_digests.iter().all(digest)
        {
            return Err(EvidenceControlError::Artifact(
                "control or provenance digest is inconsistent".into(),
            ));
        }
        if self
            .effect_receipts
            .iter()
            .any(|e| e != "block:unsafe-release" && !e.starts_with("coordinate:evidence-control:"))
        {
            return Err(EvidenceControlError::Invalid(
                "effect is outside evidence-control gate".into(),
            ));
        }
        if self.disposition == "qualified"
            && self.effect_receipts != [format!("coordinate:evidence-control:{}", self.batch_id)]
        {
            return Err(EvidenceControlError::Invalid(
                "qualified control effect is invalid".into(),
            ));
        }
        if self.disposition != "qualified" && self.effect_receipts != ["block:unsafe-release"] {
            return Err(EvidenceControlError::Invalid(
                "non-qualified control must block".into(),
            ));
        }
        Ok(())
    }
    pub fn digest(&self) -> Result<ContentHash, EvidenceControlError> {
        self.validate()?;
        ContentHash::of_value(
            &serde_json::to_value(self)
                .map_err(|e| EvidenceControlError::Artifact(e.to_string()))?,
        )
        .map_err(|e| EvidenceControlError::Artifact(e.to_string()))
    }
}
pub fn federated_evidence_control_manifest() -> serde_json::Value {
    json!({"schema_version":SCHEMA_VERSION,"capability_id":FEATURE_ID,"version":CONTRACT_VERSION,"owner_crate":"scope","consumers":["evidence workflow operator","computational biologist","federation steward"],"behavior":"admit bounded high-throughput EvidenceFeed3 batches and aggregate peer attestations under typed scope, checkpoint, quorum, policy, and locality gates","value":"coordinates federated evidence surveillance without moving raw observations and without hiding overflow or missing peers","input_schema":INPUT_SCHEMA,"output_schema":OUTPUT_SCHEMA,"effects":["coordinate:evidence-control"],"permissions":["execute:approved-workflows","exchange:permitted-summaries"],"autonomy_tier":"A2","boundary":PRECLINICAL_BOUNDARY})
}
pub fn operate_federated_evidence_control(
    request: &EvidenceControlRequest6,
) -> Result<FederatedEvidenceControlReceipt9, EvidenceControlError> {
    if request.request_id.trim().is_empty()
        || request.batch_id.trim().is_empty()
        || request.partition.trim().is_empty()
        || request.scope.trim().is_empty()
        || request.semantic_profile.trim().is_empty()
        || request.checkpoint_seq == 0
        || request.max_items == 0
        || request.max_budget_units == 0
        || request.min_peer_quorum == 0
        || !digest(&request.replay_identity)
        || !request.aggregate_only
        || !request.raw_data_local
        || request.boundary != PRECLINICAL_BOUNDARY
        || request.observations.is_empty()
        || request.peers.is_empty()
    {
        return Err(EvidenceControlError::Invalid(
            "control identity, bounds, replay, locality, peers, or boundary are invalid".into(),
        ));
    }
    let mut rows = request.observations.clone();
    rows.sort_by(|a, b| {
        a.sequence
            .cmp(&b.sequence)
            .then(a.evidence_id.cmp(&b.evidence_id))
    });
    let candidate_order = rows
        .iter()
        .map(|r| r.evidence_id.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    if candidate_order.len() != rows.len() {
        return Err(EvidenceControlError::Invalid(
            "duplicate evidence ids are invalid".into(),
        ));
    }
    let admission = request.max_items.min(request.max_budget_units as usize);
    let ranked = candidate_order.iter().cloned().collect::<BTreeSet<_>>();
    let overflow = rows
        .iter()
        .skip(admission)
        .map(|r| r.evidence_id.clone())
        .collect::<BTreeSet<_>>();
    let peer_order = request
        .peers
        .iter()
        .map(|p| p.peer_id.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let mut qualified = BTreeSet::new();
    let mut unresolved = BTreeSet::new();
    let mut blocked = BTreeSet::new();
    let mut qualified_peers = BTreeSet::new();
    let mut omissions = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    let mut negative = BTreeSet::new();
    for r in rows.iter().take(admission) {
        if r.negative_result {
            negative.insert(format!("{}:negative-result", r.evidence_id));
        }
        let hard = !request.policy_allow
            || !request.protected_closure
            || !request.federation_approved
            || !request.signed_approval
            || !r.available
            || !r.permitted
            || !r.aggregate_only
            || !r.raw_data_local
            || r.semantic_profile != request.semantic_profile
            || !digest(&r.content_digest)
            || !digest(&r.provenance_digest);
        let soft = r.replay_identity != request.replay_identity
            || r.relevance_milli < 500
            || r.freshness_milli < 500
            || !matches!(r.evidence_state.as_str(), "proven" | "supported");
        if hard || r.evidence_state == "contradicted" {
            blocked.insert(r.evidence_id.clone());
        } else if soft {
            unresolved.insert(r.evidence_id.clone());
            uncertainty.insert(format!("{}:unresolved", r.evidence_id));
        } else {
            qualified.insert(r.evidence_id.clone());
        }
    }
    if rows.len() > admission {
        omissions.insert(format!("batch:overflow:{}", rows.len() - ranked.len()));
    }
    for p in &request.peers {
        if p.available
            && p.signed
            && p.aggregate_only
            && p.raw_data_local
            && p.semantic_profile == request.semantic_profile
            && p.checkpoint_seq == request.checkpoint_seq
            && digest(&p.attestation_digest)
        {
            qualified_peers.insert(p.peer_id.clone());
        }
    }
    if qualified_peers.len() < request.min_peer_quorum {
        omissions.insert(format!(
            "peer-quorum:{}/{}",
            qualified_peers.len(),
            request.min_peer_quorum
        ));
    }
    if !request.policy_allow {
        omissions.insert("workflow:policy-denied".into());
    }
    if !request.protected_closure {
        omissions.insert("workflow:protected-closure-incomplete".into());
    }
    if !request.federation_approved {
        omissions.insert("workflow:federation-approval-missing".into());
    }
    if !request.signed_approval {
        omissions.insert("workflow:signed-approval-missing".into());
    }
    let global = !request.policy_allow
        || !request.protected_closure
        || !request.federation_approved
        || !request.signed_approval;
    let disposition = if global || !blocked.is_empty() {
        "blocked"
    } else if !overflow.is_empty()
        || !unresolved.is_empty()
        || !negative.is_empty()
        || qualified_peers.len() < request.min_peer_quorum
    {
        "unresolved"
    } else {
        "qualified"
    };
    if disposition != "qualified" {
        omissions.insert("workflow:closure-incomplete".into());
    }
    if global {
        blocked.extend(candidate_order.iter().cloned());
        qualified.clear();
        unresolved.clear();
    }
    let checkpoint_digest=ContentHash::of_value(&json!({"batch_id":request.batch_id,"checkpoint_seq":request.checkpoint_seq,"partition":request.partition,"replay_identity":request.replay_identity})).map_err(|e|EvidenceControlError::Artifact(e.to_string()))?;
    let payload = json!({"candidate_order":candidate_order,"ranked_order":ranked,"qualified_order":qualified,"unresolved_order":unresolved,"blocked_order":blocked,"overflow_order":overflow,"peer_order":peer_order,"qualified_peer_order":qualified_peers,"missing_peer_order":request.peers.iter().map(|p|p.peer_id.clone()).collect::<BTreeSet<_>>().difference(&qualified_peers).cloned().collect::<BTreeSet<_>>(),"omission_order":omissions,"uncertainty_order":uncertainty,"negative_evidence_order":negative,"checkpoint_digest":checkpoint_digest,"replay_identity":request.replay_identity});
    let control_digest = ContentHash::of_value(&payload)
        .map_err(|e| EvidenceControlError::Artifact(e.to_string()))?;
    let missing_peer_order = request
        .peers
        .iter()
        .map(|p| p.peer_id.clone())
        .filter(|p| !qualified_peers.contains(p))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let receipt = FederatedEvidenceControlReceipt9 {
        schema_version: SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        request_id: request.request_id.clone(),
        batch_id: request.batch_id.clone(),
        partition: request.partition.clone(),
        scope: request.scope.clone(),
        semantic_profile: request.semantic_profile.clone(),
        checkpoint_seq: request.checkpoint_seq,
        disposition: disposition.into(),
        candidate_order: payload["candidate_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap().into())
            .collect(),
        ranked_order: payload["ranked_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap().into())
            .collect(),
        qualified_order: payload["qualified_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap().into())
            .collect(),
        unresolved_order: payload["unresolved_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap().into())
            .collect(),
        blocked_order: payload["blocked_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap().into())
            .collect(),
        overflow_order: payload["overflow_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap().into())
            .collect(),
        peer_order: payload["peer_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap().into())
            .collect(),
        qualified_peer_order: payload["qualified_peer_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap().into())
            .collect(),
        missing_peer_order,
        omission_order: payload["omission_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap().into())
            .collect(),
        uncertainty_order: payload["uncertainty_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap().into())
            .collect(),
        negative_evidence_order: payload["negative_evidence_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap().into())
            .collect(),
        replay_identity: request.replay_identity.clone(),
        checkpoint_digest,
        control_digest: control_digest.clone(),
        artifact: FederatedEvidenceControlArtifact9 {
            schema_version: SCHEMA_VERSION.into(),
            artifact_id: format!("scope-federated-evidence-control:{}", request.batch_id),
            content_type: CONTENT_TYPE.into(),
            content_hash: control_digest,
            semantic_loss: Vec::new(),
            provenance_digests: request
                .peers
                .iter()
                .map(|p| p.attestation_digest.clone())
                .collect(),
            boundary: PRECLINICAL_BOUNDARY.into(),
        },
        effect_receipts: if disposition == "qualified" {
            vec![format!("coordinate:evidence-control:{}", request.batch_id)]
        } else {
            vec!["block:unsafe-release".into()]
        },
        raw_data_local: true,
        aggregate_only: true,
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
    fn req() -> EvidenceControlRequest6 {
        EvidenceControlRequest6 {
            request_id: "r".into(),
            batch_id: "b".into(),
            partition: "p".into(),
            scope: "scope".into(),
            semantic_profile: "profile:v1".into(),
            checkpoint_seq: 1,
            max_items: 4,
            max_budget_units: 4,
            min_peer_quorum: 1,
            replay_identity: h("replay"),
            policy_allow: true,
            protected_closure: true,
            federation_approved: true,
            signed_approval: true,
            aggregate_only: true,
            raw_data_local: true,
            boundary: PRECLINICAL_BOUNDARY.into(),
            observations: vec![ThroughputEvidence4 {
                evidence_id: "e1".into(),
                source_id: "s1".into(),
                sequence: 1,
                relevance_milli: 900,
                freshness_milli: 900,
                semantic_profile: "profile:v1".into(),
                content_digest: h("c"),
                provenance_digest: h("p"),
                replay_identity: h("replay"),
                evidence_state: "supported".into(),
                available: true,
                permitted: true,
                aggregate_only: true,
                raw_data_local: true,
                negative_result: false,
            }],
            peers: vec![PeerEvidence4 {
                peer_id: "peer1".into(),
                semantic_profile: "profile:v1".into(),
                checkpoint_seq: 1,
                attestation_digest: h("a"),
                signed: true,
                aggregate_only: true,
                raw_data_local: true,
                available: true,
            }],
        }
    }
    #[test]
    fn manifest_is_a2() {
        assert_eq!(federated_evidence_control_manifest()["autonomy_tier"], "A2")
    }
    #[test]
    fn qualified() {
        assert_eq!(
            operate_federated_evidence_control(&req())
                .unwrap()
                .disposition,
            "qualified"
        )
    }
    #[test]
    fn overflow_unresolved() {
        let mut r = req();
        r.max_items = 0;
        r.max_items = 1;
        r.observations.push(ThroughputEvidence4 {
            evidence_id: "e2".into(),
            ..r.observations[0].clone()
        });
        assert_eq!(
            operate_federated_evidence_control(&r).unwrap().disposition,
            "unresolved"
        )
    }
    #[test]
    fn policy_blocks() {
        let mut r = req();
        r.policy_allow = false;
        assert_eq!(
            operate_federated_evidence_control(&r).unwrap().disposition,
            "blocked"
        )
    }
    #[test]
    fn deterministic() {
        assert_eq!(
            operate_federated_evidence_control(&req()).unwrap(),
            operate_federated_evidence_control(&req()).unwrap()
        )
    }
}
