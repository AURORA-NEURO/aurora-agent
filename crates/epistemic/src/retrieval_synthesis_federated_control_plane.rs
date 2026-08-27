//! Prospective high-throughput retrieval/synthesis federated control plane (`AFA-epistemic-P02-F31`).
//!
//! The route ranks caller-supplied evidence summaries and peer synthesis attestations. It never
//! retrieves documents, exports raw text, or turns an unresolved evidence state into a claim.

use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-epistemic-P02-F31";
pub const CONTRACT_VERSION: &str =
    "epistemic-prospective-high-throughput-retrieval-synthesis-federated-control-plane/1.0";
pub const INPUT_SCHEMA: &str = "ScopedRetrievalQuery3@1";
pub const OUTPUT_SCHEMA: &str = "EvidenceSynthesis8@1";
pub const CONTENT_TYPE: &str = "application/vnd.aurora.evidence-synthesis-8+json";
pub const PRECLINICAL_BOUNDARY: &str = "preclinical-research-only; no human-subject or clinical-source data; no diagnosis, treatment, triage, enrollment, or clinical decisions";
pub const MAX_CANDIDATES: usize = 8192;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceState {
    Proven,
    Supported,
    Unknown,
    Unmeasured,
    Contradicted,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScopedRetrievalQuery3 {
    pub request_id: String,
    pub corpus_id: String,
    pub requester: String,
    pub purpose: String,
    pub semantic_profile: String,
    pub required_terms: Vec<String>,
    pub candidate_limit: usize,
    pub minimum_source_quorum: usize,
    pub minimum_peer_quorum: usize,
    pub budget_units: u64,
    pub checkpoint: u64,
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
pub struct RetrievalCandidate4 {
    pub evidence_id: String,
    pub source_id: String,
    pub origin: String,
    pub title: String,
    pub relevance_milli: i64,
    pub freshness_milli: i64,
    pub semantic_profile: String,
    pub terms: Vec<String>,
    pub content_digest: ContentHash,
    pub provenance_digest: ContentHash,
    pub replay_identity: ContentHash,
    pub evidence_state: EvidenceState,
    pub supported: bool,
    pub signed: bool,
    pub permitted: bool,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub negative_result: bool,
    pub omission_reasons: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PeerSynthesisSummary4 {
    pub peer_id: String,
    pub origin: String,
    pub corpus_id: String,
    pub semantic_profile: String,
    pub checkpoint: u64,
    pub synthesis_digest: ContentHash,
    pub source_count: usize,
    pub evidence_state: EvidenceState,
    pub signed: bool,
    pub aggregate_only: bool,
    pub raw_data_local: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceSynthesisArtifact8 {
    pub artifact_id: String,
    pub content_type: String,
    pub content_hash: ContentHash,
    pub semantic_loss: Vec<String>,
    pub provenance_digests: Vec<ContentHash>,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceSynthesis8 {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub corpus_id: String,
    pub requester: String,
    pub purpose: String,
    pub semantic_profile: String,
    pub checkpoint: u64,
    pub disposition: String,
    pub candidate_order: Vec<String>,
    pub qualified_order: Vec<String>,
    pub unresolved_order: Vec<String>,
    pub blocked_order: Vec<String>,
    pub source_order: Vec<String>,
    pub qualified_source_order: Vec<String>,
    pub missing_source_order: Vec<String>,
    pub peer_order: Vec<String>,
    pub qualified_peer_order: Vec<String>,
    pub missing_peer_order: Vec<String>,
    pub omission_order: Vec<String>,
    pub uncertainty_order: Vec<String>,
    pub negative_evidence_order: Vec<String>,
    pub ranked_scores_milli: Vec<i64>,
    pub replay_identity: ContentHash,
    pub synthesis_digest: ContentHash,
    pub artifact: EvidenceSynthesisArtifact8,
    pub effect_receipts: Vec<String>,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub boundary: String,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum RetrievalSynthesisError {
    #[error("invalid retrieval synthesis request: {0}")]
    Invalid(String),
    #[error("retrieval synthesis artifact failed: {0}")]
    Artifact(String),
}

pub fn retrieval_synthesis_manifest() -> serde_json::Value {
    json!({"schema_version":"aurora-research-contract/1.0","capability_id":FEATURE_ID,"version":CONTRACT_VERSION,"owner_crate":"epistemic","consumers":["downstream AURORA crate maintainer","retrieval scientist","federation steward"],"behavior":"ranks bounded typed retrieval candidates and peer synthesis summaries under evidence, replay, provenance, policy, quorum, and locality gates","value":"turns high-throughput evidence retrieval into an auditable, federated, fail-closed product capability","input_schema":INPUT_SCHEMA,"output_schema":OUTPUT_SCHEMA,"effects":["manage:local-capability","exchange:permitted-summaries"],"permissions":["operate:institution-node"],"autonomy_tier":"A2","boundary":PRECLINICAL_BOUNDARY})
}

impl EvidenceSynthesis8 {
    pub fn validate(&self) -> Result<(), RetrievalSynthesisError> {
        if self.schema_version != "aurora-research-contract/1.0"
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.boundary != PRECLINICAL_BOUNDARY
            || !self.raw_data_local
            || !self.aggregate_only
            || self.request_id.trim().is_empty()
            || self.corpus_id.trim().is_empty()
            || self.requester.trim().is_empty()
            || self.purpose.trim().is_empty()
            || self.semantic_profile.trim().is_empty()
            || self.checkpoint == 0
            || self.candidate_order.is_empty()
            || self.source_order.is_empty()
            || self.peer_order.is_empty()
            || self.effect_receipts.is_empty()
            || !["qualified", "unresolved", "blocked"].contains(&self.disposition.as_str())
        {
            return Err(RetrievalSynthesisError::Invalid("retrieval identity, checkpoint, locality, candidates, sources, peers, or effects are incomplete".into()));
        }
        for values in [
            &self.candidate_order,
            &self.qualified_order,
            &self.unresolved_order,
            &self.blocked_order,
            &self.source_order,
            &self.qualified_source_order,
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
                return Err(RetrievalSynthesisError::Invalid(
                    "retrieval synthesis ordering is not canonical".into(),
                ));
            }
        }
        let all = BTreeSet::from_iter(self.candidate_order.iter().cloned());
        let parts = self
            .qualified_order
            .iter()
            .chain(&self.unresolved_order)
            .chain(&self.blocked_order)
            .cloned()
            .collect::<BTreeSet<_>>();
        if all != parts || all.len() != self.candidate_order.len() {
            return Err(RetrievalSynthesisError::Invalid(
                "candidate dispositions do not partition".into(),
            ));
        }
        let sources = BTreeSet::from_iter(self.source_order.iter().cloned());
        let source_parts = self
            .qualified_source_order
            .iter()
            .chain(&self.missing_source_order)
            .cloned()
            .collect::<BTreeSet<_>>();
        if sources != source_parts || sources.len() != self.source_order.len() {
            return Err(RetrievalSynthesisError::Invalid(
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
            return Err(RetrievalSynthesisError::Invalid(
                "peer dispositions do not partition".into(),
            ));
        }
        if self.qualified_order.len() != self.ranked_scores_milli.len()
            || self.artifact.content_type != CONTENT_TYPE
            || self.artifact.boundary != PRECLINICAL_BOUNDARY
            || self.artifact.content_hash != self.synthesis_digest
        {
            return Err(RetrievalSynthesisError::Artifact(
                "artifact metadata, ranked scores, or digest is inconsistent".into(),
            ));
        }
        if self.effect_receipts.iter().any(|e| {
            !e.starts_with("exchange:permitted-summaries:")
                && !e.starts_with("manage:local-capability:")
                && e != "block:unsafe-release"
        }) {
            return Err(RetrievalSynthesisError::Invalid(
                "effect is outside retrieval gate".into(),
            ));
        }
        Ok(())
    }
    pub fn digest(&self) -> Result<ContentHash, RetrievalSynthesisError> {
        self.validate()?;
        ContentHash::of_value(
            &serde_json::to_value(self)
                .map_err(|e| RetrievalSynthesisError::Artifact(e.to_string()))?,
        )
        .map_err(|e| RetrievalSynthesisError::Artifact(e.to_string()))
    }
}

pub fn operate_retrieval_synthesis(
    request: &ScopedRetrievalQuery3,
    candidates: &[RetrievalCandidate4],
    peers: &[PeerSynthesisSummary4],
) -> Result<EvidenceSynthesis8, RetrievalSynthesisError> {
    validate_request(request, candidates, peers)?;
    let mut rows = candidates.to_vec();
    rows.sort_by(|a, b| {
        b.relevance_milli
            .cmp(&a.relevance_milli)
            .then(b.freshness_milli.cmp(&a.freshness_milli))
            .then(a.evidence_id.cmp(&b.evidence_id))
    });
    let candidate_order = rows
        .iter()
        .map(|r| r.evidence_id.clone())
        .collect::<Vec<_>>();
    let mut peer_rows = peers.to_vec();
    peer_rows.sort_by(|a, b| a.peer_id.cmp(&b.peer_id));
    let peer_order = peer_rows
        .iter()
        .map(|p| p.peer_id.clone())
        .collect::<Vec<_>>();
    let mut qualified_peers = BTreeSet::new();
    let mut missing_peers = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    for p in &peer_rows {
        let ok = p.corpus_id == request.corpus_id
            && p.semantic_profile == request.semantic_profile
            && p.checkpoint == request.checkpoint
            && p.source_count >= request.minimum_source_quorum
            && p.signed
            && p.aggregate_only
            && p.raw_data_local
            && matches!(
                p.evidence_state,
                EvidenceState::Proven | EvidenceState::Supported
            );
        if ok {
            qualified_peers.insert(p.peer_id.clone());
        } else {
            missing_peers.insert(p.peer_id.clone());
            uncertainty.insert(format!("peer:{}:not-qualified", p.peer_id));
        }
    }
    let mut qualified = BTreeSet::new();
    let mut unresolved = BTreeSet::new();
    let mut blocked = BTreeSet::new();
    let mut source_order = rows
        .iter()
        .map(|r| r.source_id.clone())
        .collect::<BTreeSet<_>>();
    let mut qualified_sources = BTreeSet::new();
    let mut omissions = BTreeSet::new();
    let mut negative = BTreeSet::new();
    let mut scores = Vec::new();
    for r in &rows {
        if r.negative_result {
            negative.insert(format!("{}:negative-result", r.evidence_id));
        }
        for x in &r.omission_reasons {
            omissions.insert(format!("{}:{}", r.evidence_id, x));
        }
        let missing_terms = request
            .required_terms
            .iter()
            .filter(|t| !r.terms.contains(t))
            .count();
        let mut reasons = Vec::new();
        if r.semantic_profile != request.semantic_profile {
            reasons.push("semantic-profile-mismatch");
        }
        if missing_terms > 0 {
            reasons.push("required-term-missing");
            omissions.insert(format!("{}:missing-terms:{}", r.evidence_id, missing_terms));
        }
        if r.replay_identity != request.replay_identity {
            reasons.push("replay-identity-mismatch");
        }
        if !r.signed || !r.permitted {
            reasons.push("authorization-missing");
        }
        if !r.raw_data_local || !r.aggregate_only {
            reasons.push("locality-or-aggregate-only-failed");
        }
        if r.evidence_state == EvidenceState::Contradicted {
            blocked.insert(r.evidence_id.clone());
            negative.insert(format!("{}:contradicted", r.evidence_id));
        } else if !matches!(
            r.evidence_state,
            EvidenceState::Proven | EvidenceState::Supported
        ) || !reasons.is_empty()
        {
            unresolved.insert(r.evidence_id.clone());
            uncertainty.insert(format!("{}:unresolved", r.evidence_id));
        } else {
            qualified.insert(r.evidence_id.clone());
            qualified_sources.insert(r.source_id.clone());
            scores.push(r.relevance_milli.saturating_add(r.freshness_milli));
        }
    }
    let global_block = !request.policy_allow
        || !request.protected_closure
        || !request.signed_approval
        || !request.federation_approved
        || !request.raw_data_local
        || !request.aggregate_only;
    if !request.policy_allow {
        negative.insert("request:policy-denied".into());
    }
    if !request.protected_closure {
        uncertainty.insert("request:protected-closure-incomplete".into());
    }
    if !request.signed_approval {
        uncertainty.insert("request:signed-approval-missing".into());
    }
    if !request.federation_approved {
        uncertainty.insert("request:federation-approval-missing".into());
    }
    if qualified_sources.len() < request.minimum_source_quorum {
        uncertainty.insert("source:minimum-quorum-unmet".into());
    }
    let disposition = if global_block || !blocked.is_empty() {
        "blocked"
    } else if qualified.is_empty()
        || qualified_sources.len() < request.minimum_source_quorum
        || qualified_peers.len() < request.minimum_peer_quorum
    {
        "unresolved"
    } else {
        "qualified"
    };
    if global_block {
        blocked.extend(candidate_order.iter().cloned());
        qualified.clear();
        unresolved.clear();
        scores.clear();
    }
    if disposition != "qualified" {
        omissions.insert("request:retrieval-gates-incomplete".into());
    }
    let qualified_order = qualified.into_iter().collect::<Vec<_>>();
    let unresolved_order = unresolved.into_iter().collect::<Vec<_>>();
    let blocked_order = blocked.into_iter().collect::<Vec<_>>();
    let source_order = source_order.into_iter().collect::<Vec<_>>();
    let qualified_source_order = qualified_sources.into_iter().collect::<Vec<_>>();
    let missing_source_order = source_order
        .iter()
        .filter(|x| !qualified_source_order.contains(x))
        .cloned()
        .collect::<Vec<_>>();
    let payload = json!({"schema_version":"aurora-research-contract/1.0","contract_version":CONTRACT_VERSION,"feature_id":FEATURE_ID,"request_id":request.request_id,"corpus_id":request.corpus_id,"requester":request.requester,"purpose":request.purpose,"semantic_profile":request.semantic_profile,"checkpoint":request.checkpoint,"disposition":disposition,"candidate_order":candidate_order,"qualified_order":qualified_order,"unresolved_order":unresolved_order,"blocked_order":blocked_order,"source_order":source_order,"qualified_source_order":qualified_source_order,"missing_source_order":missing_source_order,"peer_order":peer_order,"qualified_peer_order":qualified_peers,"missing_peer_order":missing_peers,"omission_order":omissions,"uncertainty_order":uncertainty,"negative_evidence_order":negative,"ranked_scores_milli":scores,"replay_identity":request.replay_identity,"boundary":PRECLINICAL_BOUNDARY});
    let synthesis_digest = ContentHash::of_value(&payload)
        .map_err(|e| RetrievalSynthesisError::Artifact(e.to_string()))?;
    let artifact = EvidenceSynthesisArtifact8 {
        artifact_id: format!("evidence-synthesis-8:{}", request.request_id),
        content_type: CONTENT_TYPE.into(),
        content_hash: synthesis_digest.clone(),
        semantic_loss: Vec::new(),
        provenance_digests: rows
            .iter()
            .map(|r| r.provenance_digest.clone())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect(),
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    let effects = if disposition == "qualified" {
        vec![
            format!("exchange:permitted-summaries:{}", request.request_id),
            format!("manage:local-capability:{}", request.request_id),
        ]
    } else {
        vec!["block:unsafe-release".into()]
    };
    let receipt = EvidenceSynthesis8 {
        schema_version: "aurora-research-contract/1.0".into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        request_id: request.request_id.clone(),
        corpus_id: request.corpus_id.clone(),
        requester: request.requester.clone(),
        purpose: request.purpose.clone(),
        semantic_profile: request.semantic_profile.clone(),
        checkpoint: request.checkpoint,
        disposition: disposition.into(),
        candidate_order,
        qualified_order,
        unresolved_order,
        blocked_order,
        source_order,
        qualified_source_order,
        missing_source_order,
        peer_order,
        qualified_peer_order: qualified_peers.into_iter().collect(),
        missing_peer_order: missing_peers.into_iter().collect(),
        omission_order: omissions.into_iter().collect(),
        uncertainty_order: uncertainty.into_iter().collect(),
        negative_evidence_order: negative.into_iter().collect(),
        ranked_scores_milli: scores,
        replay_identity: request.replay_identity.clone(),
        synthesis_digest,
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
    request: &ScopedRetrievalQuery3,
    candidates: &[RetrievalCandidate4],
    peers: &[PeerSynthesisSummary4],
) -> Result<(), RetrievalSynthesisError> {
    if request.request_id.trim().is_empty()
        || request.corpus_id.trim().is_empty()
        || request.requester.trim().is_empty()
        || request.purpose.trim().is_empty()
        || request.semantic_profile.trim().is_empty()
        || request.required_terms.is_empty()
        || request.candidate_limit == 0
        || request.checkpoint == 0
        || request.minimum_source_quorum == 0
        || request.minimum_peer_quorum == 0
        || request.budget_units == 0
        || request.boundary != PRECLINICAL_BOUNDARY
        || !request.raw_data_local
        || !request.aggregate_only
        || request.replay_identity.as_str().len() != 64
        || candidates.is_empty()
        || candidates.len() > MAX_CANDIDATES
        || peers.is_empty()
    {
        return Err(RetrievalSynthesisError::Invalid("request identity, terms, bounds, checkpoint, budget, locality, candidates, peers, replay, or boundary is invalid".into()));
    }
    let mut ids = BTreeSet::new();
    for c in candidates {
        if c.evidence_id.trim().is_empty()
            || c.source_id.trim().is_empty()
            || c.origin.trim().is_empty()
            || c.title.trim().is_empty()
            || !ids.insert(c.evidence_id.clone())
            || c.content_digest.as_str().len() != 64
            || c.provenance_digest.as_str().len() != 64
            || c.replay_identity.as_str().len() != 64
        {
            return Err(RetrievalSynthesisError::Invalid(
                "candidate identity, uniqueness, origin, title, or digest is invalid".into(),
            ));
        }
    }
    let mut pids = BTreeSet::new();
    for p in peers {
        if p.peer_id.trim().is_empty()
            || !pids.insert(p.peer_id.clone())
            || p.origin.trim().is_empty()
            || p.synthesis_digest.as_str().len() != 64
        {
            return Err(RetrievalSynthesisError::Invalid(
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
    fn req() -> ScopedRetrievalQuery3 {
        ScopedRetrievalQuery3 {
            request_id: "request:retrieval".into(),
            corpus_id: "corpus:1".into(),
            requester: "crate-maintainer".into(),
            purpose: "synthesis".into(),
            semantic_profile: "neuro:v1".into(),
            required_terms: vec!["neuron".into()],
            candidate_limit: 4,
            minimum_source_quorum: 1,
            minimum_peer_quorum: 1,
            budget_units: 10,
            checkpoint: 1,
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
    fn cand(id: &str, state: EvidenceState) -> RetrievalCandidate4 {
        RetrievalCandidate4 {
            evidence_id: id.into(),
            source_id: format!("source:{id}"),
            origin: "site-a".into(),
            title: "neuron study".into(),
            relevance_milli: 90,
            freshness_milli: 80,
            semantic_profile: "neuro:v1".into(),
            terms: vec!["neuron".into()],
            content_digest: h(id),
            provenance_digest: h(&format!("p:{id}")),
            replay_identity: h("r"),
            evidence_state: state,
            supported: true,
            signed: true,
            permitted: true,
            raw_data_local: true,
            aggregate_only: true,
            negative_result: false,
            omission_reasons: Vec::new(),
        }
    }
    fn peer() -> PeerSynthesisSummary4 {
        PeerSynthesisSummary4 {
            peer_id: "peer:a".into(),
            origin: "site-a".into(),
            corpus_id: "corpus:1".into(),
            semantic_profile: "neuro:v1".into(),
            checkpoint: 1,
            synthesis_digest: h("peer"),
            source_count: 1,
            evidence_state: EvidenceState::Supported,
            signed: true,
            aggregate_only: true,
            raw_data_local: true,
        }
    }
    #[test]
    fn manifest_is_a2() {
        assert_eq!(retrieval_synthesis_manifest()["autonomy_tier"], "A2");
    }
    #[test]
    fn qualified_is_deterministic() {
        let r = operate_retrieval_synthesis(
            &req(),
            &[
                cand("b", EvidenceState::Supported),
                cand("a", EvidenceState::Proven),
            ],
            &[peer()],
        )
        .unwrap();
        assert_eq!(r.disposition, "qualified");
        assert_eq!(r.digest().unwrap(), r.digest().unwrap());
    }
    #[test]
    fn unknown_is_unresolved() {
        let r =
            operate_retrieval_synthesis(&req(), &[cand("a", EvidenceState::Unknown)], &[peer()])
                .unwrap();
        assert_eq!(r.disposition, "unresolved");
        assert!(!r.uncertainty_order.is_empty());
    }
    #[test]
    fn contradiction_blocks() {
        let r = operate_retrieval_synthesis(
            &req(),
            &[cand("a", EvidenceState::Contradicted)],
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
        let r = operate_retrieval_synthesis(&q, &[cand("a", EvidenceState::Supported)], &[peer()])
            .unwrap();
        assert_eq!(r.disposition, "blocked");
    }
    #[test]
    fn duplicate_is_rejected() {
        assert!(operate_retrieval_synthesis(
            &req(),
            &[
                cand("a", EvidenceState::Supported),
                cand("a", EvidenceState::Supported)
            ],
            &[peer()]
        )
        .is_err());
    }
}
