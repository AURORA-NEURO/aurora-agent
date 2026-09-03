//! Multimodal knowledge-representation federated control plane (`AFA-ids-P04-F30`).
//!
//! The compiler admits typed claim summaries into a content-addressed world view. It is
//! deliberately a summary boundary: no raw imaging/omics payloads are read or exported and
//! contradictory or unknown claims remain explicit.

use crate::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-ids-P04-F30";
pub const CONTRACT_VERSION: &str =
    "ids-multimodal-knowledge-representation-federated-control-plane/1.0";
pub const INPUT_SCHEMA: &str = "ScopedKnowledgeClaims4@1";
pub const OUTPUT_SCHEMA: &str = "TypedKnowledgeWorld7@1";
pub const CONTENT_TYPE: &str = "application/vnd.aurora.typed-knowledge-world-7+json";
pub const PRECLINICAL_BOUNDARY: &str = "preclinical-research-only; no human-subject or clinical-source data; no diagnosis, treatment, triage, enrollment, or clinical decisions";

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum KnowledgeEvidenceState {
    Proven,
    Supported,
    Unknown,
    Unmeasured,
    Contradicted,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScopedKnowledgeClaims4 {
    pub request_id: String,
    pub world_id: String,
    pub federation_id: String,
    pub requester: String,
    pub purpose: String,
    pub semantic_profile: String,
    pub required_claims: Vec<String>,
    pub minimum_source_quorum: usize,
    pub minimum_peer_quorum: usize,
    pub checkpoint: u64,
    pub budget_units: u64,
    pub replay_identity: ContentHash,
    pub policy_allow: bool,
    pub protected_closure: bool,
    pub signed_approval: bool,
    pub federation_approved: bool,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KnowledgeClaim4 {
    pub claim_id: String,
    pub subject: String,
    pub predicate: String,
    pub object: String,
    pub source_id: String,
    pub origin: String,
    pub modality: String,
    pub confidence_milli: i64,
    pub semantic_profile: String,
    pub terms: Vec<String>,
    pub content_digest: ContentHash,
    pub provenance_digest: ContentHash,
    pub replay_identity: ContentHash,
    pub evidence_state: KnowledgeEvidenceState,
    pub signed: bool,
    pub permitted: bool,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub negative_result: bool,
    pub omission_reasons: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KnowledgePeer4 {
    pub peer_id: String,
    pub origin: String,
    pub federation_id: String,
    pub world_id: String,
    pub semantic_profile: String,
    pub checkpoint: u64,
    pub world_digest: ContentHash,
    pub source_count: usize,
    pub evidence_state: KnowledgeEvidenceState,
    pub signed: bool,
    pub aggregate_only: bool,
    pub raw_data_local: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TypedKnowledgeWorld7Artifact {
    pub artifact_id: String,
    pub content_type: String,
    pub content_hash: ContentHash,
    pub semantic_loss: Vec<String>,
    pub provenance_digests: Vec<ContentHash>,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TypedKnowledgeWorld7 {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub world_id: String,
    pub federation_id: String,
    pub requester: String,
    pub purpose: String,
    pub semantic_profile: String,
    pub checkpoint: u64,
    pub disposition: String,
    pub claim_order: Vec<String>,
    pub selected_claim_order: Vec<String>,
    pub unresolved_claim_order: Vec<String>,
    pub blocked_claim_order: Vec<String>,
    pub source_order: Vec<String>,
    pub selected_source_order: Vec<String>,
    pub missing_source_order: Vec<String>,
    pub peer_order: Vec<String>,
    pub qualified_peer_order: Vec<String>,
    pub missing_peer_order: Vec<String>,
    pub omission_order: Vec<String>,
    pub uncertainty_order: Vec<String>,
    pub negative_evidence_order: Vec<String>,
    pub confidence_scores_milli: Vec<i64>,
    pub replay_identity: ContentHash,
    pub world_digest: ContentHash,
    pub artifact: TypedKnowledgeWorld7Artifact,
    pub effect_receipts: Vec<String>,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub boundary: String,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum KnowledgeRepresentationError {
    #[error("invalid knowledge representation request: {0}")]
    Invalid(String),
    #[error("knowledge representation artifact failed: {0}")]
    Artifact(String),
}

pub fn knowledge_representation_manifest() -> serde_json::Value {
    json!({"schema_version":"aurora-research-contract/1.0","capability_id":FEATURE_ID,"version":CONTRACT_VERSION,"owner_crate":"ids","consumers":["knowledge engineer","computational biologist","federation steward"],"behavior":"compiles bounded multimodal typed claims and peer world attestations into a content-addressed world view","value":"makes knowledge representation reproducible, comparable, and fail-closed across research institutions","input_schema":INPUT_SCHEMA,"output_schema":OUTPUT_SCHEMA,"effects":["exchange:permitted-knowledge-world","manage:local-capability"],"permissions":["read:local-research-artifacts","operate:institution-node"],"autonomy_tier":"A2","boundary":PRECLINICAL_BOUNDARY})
}

impl TypedKnowledgeWorld7 {
    pub fn validate(&self) -> Result<(), KnowledgeRepresentationError> {
        if self.schema_version != "aurora-research-contract/1.0"
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.boundary != PRECLINICAL_BOUNDARY
            || !self.raw_data_local
            || !self.aggregate_only
            || self.request_id.trim().is_empty()
            || self.world_id.trim().is_empty()
            || self.federation_id.trim().is_empty()
            || self.requester.trim().is_empty()
            || self.purpose.trim().is_empty()
            || self.semantic_profile.trim().is_empty()
            || self.checkpoint == 0
            || self.claim_order.is_empty()
            || self.source_order.is_empty()
            || self.peer_order.is_empty()
            || self.effect_receipts.is_empty()
            || !["qualified", "unresolved", "blocked"].contains(&self.disposition.as_str())
        {
            return Err(KnowledgeRepresentationError::Invalid("knowledge identity, checkpoint, locality, claims, sources, peers, or effects are incomplete".into()));
        }
        for values in [
            &self.claim_order,
            &self.selected_claim_order,
            &self.unresolved_claim_order,
            &self.blocked_claim_order,
            &self.source_order,
            &self.selected_source_order,
            &self.missing_source_order,
            &self.peer_order,
            &self.qualified_peer_order,
            &self.missing_peer_order,
            &self.omission_order,
            &self.uncertainty_order,
            &self.negative_evidence_order,
            &self.effect_receipts,
        ] {
            if values.windows(2).any(|w| w[0] >= w[1]) {
                return Err(KnowledgeRepresentationError::Invalid(
                    "knowledge ordering is not canonical".into(),
                ));
            }
        }
        let claims = BTreeSet::from_iter(self.claim_order.iter().cloned());
        let parts = self
            .selected_claim_order
            .iter()
            .chain(&self.unresolved_claim_order)
            .chain(&self.blocked_claim_order)
            .cloned()
            .collect::<BTreeSet<_>>();
        if claims != parts || claims.len() != self.claim_order.len() {
            return Err(KnowledgeRepresentationError::Invalid(
                "claim dispositions do not partition".into(),
            ));
        }
        let sources = BTreeSet::from_iter(self.source_order.iter().cloned());
        let source_parts = self
            .selected_source_order
            .iter()
            .chain(&self.missing_source_order)
            .cloned()
            .collect::<BTreeSet<_>>();
        if sources != source_parts || sources.len() != self.source_order.len() {
            return Err(KnowledgeRepresentationError::Invalid(
                "source dispositions do not partition".into(),
            ));
        }
        let peers = BTreeSet::from_iter(self.peer_order.iter().cloned());
        let peer_parts = self
            .qualified_peer_order
            .iter()
            .chain(&self.missing_peer_order)
            .cloned()
            .collect::<BTreeSet<_>>();
        if peers != peer_parts || peers.len() != self.peer_order.len() {
            return Err(KnowledgeRepresentationError::Invalid(
                "peer dispositions do not partition".into(),
            ));
        }
        if self.selected_claim_order.len() != self.confidence_scores_milli.len()
            || self.artifact.content_type != CONTENT_TYPE
            || self.artifact.boundary != PRECLINICAL_BOUNDARY
            || self.artifact.content_hash != self.world_digest
        {
            return Err(KnowledgeRepresentationError::Artifact(
                "artifact metadata, score cardinality, or digest is inconsistent".into(),
            ));
        }
        if self.effect_receipts.iter().any(|e| {
            !e.starts_with("exchange:permitted-knowledge-world:")
                && !e.starts_with("manage:local-capability:")
                && e != "block:unsafe-release"
        }) {
            return Err(KnowledgeRepresentationError::Invalid(
                "effect is outside knowledge gate".into(),
            ));
        }
        Ok(())
    }
    pub fn digest(&self) -> Result<ContentHash, KnowledgeRepresentationError> {
        self.validate()?;
        ContentHash::of_value(
            &serde_json::to_value(self)
                .map_err(|e| KnowledgeRepresentationError::Artifact(e.to_string()))?,
        )
        .map_err(|e| KnowledgeRepresentationError::Artifact(e.to_string()))
    }
}

pub fn operate_knowledge_representation(
    request: &ScopedKnowledgeClaims4,
    claims: &[KnowledgeClaim4],
    peers: &[KnowledgePeer4],
) -> Result<TypedKnowledgeWorld7, KnowledgeRepresentationError> {
    validate_request(request, claims, peers)?;
    let mut rows = claims.to_vec();
    rows.sort_by(|a, b| {
        b.confidence_milli
            .cmp(&a.confidence_milli)
            .then(a.claim_id.cmp(&b.claim_id))
    });
    let claim_order = rows.iter().map(|x| x.claim_id.clone()).collect::<Vec<_>>();
    let mut peer_rows = peers.to_vec();
    peer_rows.sort_by(|a, b| a.peer_id.cmp(&b.peer_id));
    let peer_order = peer_rows
        .iter()
        .map(|x| x.peer_id.clone())
        .collect::<Vec<_>>();
    let mut qp = BTreeSet::new();
    let mut mp = BTreeSet::new();
    let mut unc = BTreeSet::new();
    for p in &peer_rows {
        let ok = p.federation_id == request.federation_id
            && p.world_id == request.world_id
            && p.semantic_profile == request.semantic_profile
            && p.checkpoint == request.checkpoint
            && p.source_count >= request.minimum_source_quorum
            && p.signed
            && p.aggregate_only
            && p.raw_data_local
            && matches!(
                p.evidence_state,
                KnowledgeEvidenceState::Proven | KnowledgeEvidenceState::Supported
            );
        if ok {
            qp.insert(p.peer_id.clone());
        } else {
            mp.insert(p.peer_id.clone());
            unc.insert(format!("peer:{}:not-qualified", p.peer_id));
        }
    }
    let mut selected = BTreeSet::new();
    let mut unresolved = BTreeSet::new();
    let mut blocked = BTreeSet::new();
    let mut sources = BTreeSet::new();
    let mut omission = BTreeSet::new();
    let mut negative = BTreeSet::new();
    let mut scores = Vec::new();
    for c in &rows {
        if c.negative_result {
            negative.insert(format!("{}:negative-result", c.claim_id));
        }
        for r in &c.omission_reasons {
            omission.insert(format!("{}:{}", c.claim_id, r));
        }
        let mut reasons = Vec::new();
        if c.semantic_profile != request.semantic_profile {
            reasons.push("semantic-profile-mismatch");
        }
        let missing = request
            .required_claims
            .iter()
            .filter(|t| !c.terms.contains(t))
            .count();
        if missing > 0 {
            reasons.push("required-claim-missing");
            omission.insert(format!("{}:missing-claims:{}", c.claim_id, missing));
        }
        if c.replay_identity != request.replay_identity {
            reasons.push("replay-identity-mismatch");
        }
        if !c.signed || !c.permitted {
            reasons.push("authorization-missing");
        }
        if !c.raw_data_local || !c.aggregate_only {
            reasons.push("locality-or-aggregate-only-failed");
        }
        if c.evidence_state == KnowledgeEvidenceState::Contradicted {
            blocked.insert(c.claim_id.clone());
            negative.insert(format!("{}:contradicted", c.claim_id));
        } else if !matches!(
            c.evidence_state,
            KnowledgeEvidenceState::Proven | KnowledgeEvidenceState::Supported
        ) || !reasons.is_empty()
        {
            unresolved.insert(c.claim_id.clone());
            unc.insert(format!("{}:unresolved", c.claim_id));
        } else {
            selected.insert(c.claim_id.clone());
            sources.insert(c.source_id.clone());
            scores.push(c.confidence_milli);
        }
    }
    let global = !request.policy_allow
        || !request.protected_closure
        || !request.signed_approval
        || !request.federation_approved
        || !request.raw_data_local
        || !request.aggregate_only;
    if !request.policy_allow {
        negative.insert("request:policy-denied".into());
    }
    if !request.protected_closure {
        unc.insert("request:protected-closure-incomplete".into());
    }
    if !request.signed_approval {
        unc.insert("request:signed-approval-missing".into());
    }
    if !request.federation_approved {
        unc.insert("request:federation-approval-missing".into());
    }
    if sources.len() < request.minimum_source_quorum {
        unc.insert("source:minimum-quorum-unmet".into());
    }
    let disposition = if global || !blocked.is_empty() {
        "blocked"
    } else if selected.is_empty()
        || sources.len() < request.minimum_source_quorum
        || qp.len() < request.minimum_peer_quorum
    {
        "unresolved"
    } else {
        "qualified"
    };
    if global {
        blocked.extend(claim_order.iter().cloned());
        selected.clear();
        unresolved.clear();
        scores.clear();
    }
    if disposition != "qualified" {
        omission.insert("request:knowledge-gates-incomplete".into());
    }
    let selected_order = selected.into_iter().collect::<Vec<_>>();
    let unresolved_order = unresolved.into_iter().collect::<Vec<_>>();
    let blocked_order = blocked.into_iter().collect::<Vec<_>>();
    let source_order = rows
        .iter()
        .map(|x| x.source_id.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let selected_source_order = sources.into_iter().collect::<Vec<_>>();
    let missing_source_order = source_order
        .iter()
        .filter(|x| !selected_source_order.contains(x))
        .cloned()
        .collect::<Vec<_>>();
    let payload = json!({"schema_version":"aurora-research-contract/1.0","contract_version":CONTRACT_VERSION,"feature_id":FEATURE_ID,"request_id":request.request_id,"world_id":request.world_id,"federation_id":request.federation_id,"requester":request.requester,"purpose":request.purpose,"semantic_profile":request.semantic_profile,"checkpoint":request.checkpoint,"disposition":disposition,"claim_order":claim_order,"selected_claim_order":selected_order,"unresolved_claim_order":unresolved_order,"blocked_claim_order":blocked_order,"source_order":source_order,"selected_source_order":selected_source_order,"missing_source_order":missing_source_order,"peer_order":peer_order,"qualified_peer_order":qp,"missing_peer_order":mp,"omission_order":omission,"uncertainty_order":unc,"negative_evidence_order":negative,"confidence_scores_milli":scores,"replay_identity":request.replay_identity,"boundary":PRECLINICAL_BOUNDARY});
    let world_digest = ContentHash::of_value(&payload)
        .map_err(|e| KnowledgeRepresentationError::Artifact(e.to_string()))?;
    let artifact = TypedKnowledgeWorld7Artifact {
        artifact_id: format!("typed-knowledge-world-7:{}", request.request_id),
        content_type: CONTENT_TYPE.into(),
        content_hash: world_digest.clone(),
        semantic_loss: Vec::new(),
        provenance_digests: rows
            .iter()
            .map(|x| x.provenance_digest.clone())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect(),
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    let effects = if disposition == "qualified" {
        vec![
            format!("exchange:permitted-knowledge-world:{}", request.request_id),
            format!("manage:local-capability:{}", request.request_id),
        ]
    } else {
        vec!["block:unsafe-release".into()]
    };
    let receipt = TypedKnowledgeWorld7 {
        schema_version: "aurora-research-contract/1.0".into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        request_id: request.request_id.clone(),
        world_id: request.world_id.clone(),
        federation_id: request.federation_id.clone(),
        requester: request.requester.clone(),
        purpose: request.purpose.clone(),
        semantic_profile: request.semantic_profile.clone(),
        checkpoint: request.checkpoint,
        disposition: disposition.into(),
        claim_order: payload["claim_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|x| x.as_str().unwrap().into())
            .collect(),
        selected_claim_order: payload["selected_claim_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|x| x.as_str().unwrap().into())
            .collect(),
        unresolved_claim_order: payload["unresolved_claim_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|x| x.as_str().unwrap().into())
            .collect(),
        blocked_claim_order: payload["blocked_claim_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|x| x.as_str().unwrap().into())
            .collect(),
        source_order: payload["source_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|x| x.as_str().unwrap().into())
            .collect(),
        selected_source_order: payload["selected_source_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|x| x.as_str().unwrap().into())
            .collect(),
        missing_source_order: payload["missing_source_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|x| x.as_str().unwrap().into())
            .collect(),
        peer_order: payload["peer_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|x| x.as_str().unwrap().into())
            .collect(),
        qualified_peer_order: payload["qualified_peer_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|x| x.as_str().unwrap().into())
            .collect(),
        missing_peer_order: payload["missing_peer_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|x| x.as_str().unwrap().into())
            .collect(),
        omission_order: payload["omission_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|x| x.as_str().unwrap().into())
            .collect(),
        uncertainty_order: payload["uncertainty_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|x| x.as_str().unwrap().into())
            .collect(),
        negative_evidence_order: payload["negative_evidence_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|x| x.as_str().unwrap().into())
            .collect(),
        confidence_scores_milli: scores,
        replay_identity: request.replay_identity.clone(),
        world_digest,
        artifact,
        effect_receipts: effects,
        raw_data_local: request.raw_data_local,
        aggregate_only: request.aggregate_only,
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    receipt.validate()?;
    Ok(receipt)
}

fn validate_request(
    request: &ScopedKnowledgeClaims4,
    claims: &[KnowledgeClaim4],
    peers: &[KnowledgePeer4],
) -> Result<(), KnowledgeRepresentationError> {
    if request.request_id.trim().is_empty()
        || request.world_id.trim().is_empty()
        || request.federation_id.trim().is_empty()
        || request.requester.trim().is_empty()
        || request.purpose.trim().is_empty()
        || request.semantic_profile.trim().is_empty()
        || request.required_claims.is_empty()
        || request.minimum_source_quorum == 0
        || request.minimum_peer_quorum == 0
        || request.checkpoint == 0
        || request.budget_units == 0
        || request.replay_identity.as_str().len() != 64
        || request.boundary != PRECLINICAL_BOUNDARY
        || !request.raw_data_local
        || !request.aggregate_only
        || claims.is_empty()
        || peers.is_empty()
    {
        return Err(KnowledgeRepresentationError::Invalid("request identity, claims, quorum, checkpoint, budget, replay, locality, peers, or boundary is invalid".into()));
    }
    let mut ids = BTreeSet::new();
    for c in claims {
        if c.claim_id.trim().is_empty()
            || c.subject.trim().is_empty()
            || c.predicate.trim().is_empty()
            || c.object.trim().is_empty()
            || c.source_id.trim().is_empty()
            || c.origin.trim().is_empty()
            || c.content_digest.as_str().len() != 64
            || c.provenance_digest.as_str().len() != 64
            || c.replay_identity.as_str().len() != 64
            || !ids.insert(c.claim_id.clone())
        {
            return Err(KnowledgeRepresentationError::Invalid(
                "claim identity, uniqueness, terms, origin, or digest is invalid".into(),
            ));
        }
    }
    let mut pids = BTreeSet::new();
    for p in peers {
        if p.peer_id.trim().is_empty()
            || p.origin.trim().is_empty()
            || p.world_digest.as_str().len() != 64
            || !pids.insert(p.peer_id.clone())
        {
            return Err(KnowledgeRepresentationError::Invalid(
                "peer identity, uniqueness, origin, or digest is invalid".into(),
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn h(s: &str) -> ContentHash {
        ContentHash::of_bytes(s.as_bytes())
    }
    fn req() -> ScopedKnowledgeClaims4 {
        ScopedKnowledgeClaims4 {
            request_id: "request:world".into(),
            world_id: "world:1".into(),
            federation_id: "federation:1".into(),
            requester: "knowledge-engineer".into(),
            purpose: "world-model".into(),
            semantic_profile: "multi:v1".into(),
            required_claims: vec!["neuron".into()],
            minimum_source_quorum: 1,
            minimum_peer_quorum: 1,
            checkpoint: 1,
            budget_units: 10,
            replay_identity: h("r"),
            policy_allow: true,
            protected_closure: true,
            signed_approval: true,
            federation_approved: true,
            raw_data_local: true,
            aggregate_only: true,
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }
    fn claim(id: &str, state: KnowledgeEvidenceState) -> KnowledgeClaim4 {
        KnowledgeClaim4 {
            claim_id: id.into(),
            subject: "cell".into(),
            predicate: "has-state".into(),
            object: "neuron".into(),
            source_id: format!("source:{id}"),
            origin: "site-a".into(),
            modality: "imaging".into(),
            confidence_milli: 90,
            semantic_profile: "multi:v1".into(),
            terms: vec!["neuron".into()],
            content_digest: h(id),
            provenance_digest: h(&format!("p:{id}")),
            replay_identity: h("r"),
            evidence_state: state,
            signed: true,
            permitted: true,
            raw_data_local: true,
            aggregate_only: true,
            negative_result: false,
            omission_reasons: Vec::new(),
        }
    }
    fn peer() -> KnowledgePeer4 {
        KnowledgePeer4 {
            peer_id: "peer:a".into(),
            origin: "site-a".into(),
            federation_id: "federation:1".into(),
            world_id: "world:1".into(),
            semantic_profile: "multi:v1".into(),
            checkpoint: 1,
            world_digest: h("peer"),
            source_count: 1,
            evidence_state: KnowledgeEvidenceState::Supported,
            signed: true,
            aggregate_only: true,
            raw_data_local: true,
        }
    }
    #[test]
    fn manifest_is_a2() {
        assert_eq!(knowledge_representation_manifest()["autonomy_tier"], "A2");
    }
    #[test]
    fn qualified_is_replayable() {
        let r = operate_knowledge_representation(
            &req(),
            &[
                claim("b", KnowledgeEvidenceState::Supported),
                claim("a", KnowledgeEvidenceState::Proven),
            ],
            &[peer()],
        )
        .unwrap();
        assert_eq!(r.disposition, "qualified");
        assert_eq!(r.digest().unwrap(), r.digest().unwrap());
    }
    #[test]
    fn unknown_is_unresolved() {
        let r = operate_knowledge_representation(
            &req(),
            &[claim("a", KnowledgeEvidenceState::Unknown)],
            &[peer()],
        )
        .unwrap();
        assert_eq!(r.disposition, "unresolved");
    }
    #[test]
    fn contradiction_blocks() {
        let r = operate_knowledge_representation(
            &req(),
            &[claim("a", KnowledgeEvidenceState::Contradicted)],
            &[peer()],
        )
        .unwrap();
        assert_eq!(r.disposition, "blocked");
        assert!(!r.negative_evidence_order.is_empty());
    }
    #[test]
    fn policy_denial_blocks() {
        let mut q = req();
        q.policy_allow = false;
        let r = operate_knowledge_representation(
            &q,
            &[claim("a", KnowledgeEvidenceState::Supported)],
            &[peer()],
        )
        .unwrap();
        assert_eq!(r.disposition, "blocked");
    }
    #[test]
    fn duplicate_is_rejected() {
        assert!(operate_knowledge_representation(
            &req(),
            &[
                claim("a", KnowledgeEvidenceState::Supported),
                claim("a", KnowledgeEvidenceState::Supported)
            ],
            &[peer()]
        )
        .is_err());
    }
}
