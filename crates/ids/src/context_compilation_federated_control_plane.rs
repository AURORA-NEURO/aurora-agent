//! Federated continual context compilation inference engine (`AFA-ids-P03-F04`).
//!
//! This is a bounded, local-first context compiler. It consumes typed fact summaries and
//! aggregate-only peer attestations; it never fetches documents, moves raw observations, or
//! converts an unresolved claim into a certified decision section.

use crate::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-ids-P03-F04";
pub const CONTRACT_VERSION: &str =
    "ids-federated-continual-context-compilation-inference-engine/1.0";
pub const INPUT_SCHEMA: &str = "DecisionQuery4@1";
pub const OUTPUT_SCHEMA: &str = "CertifiedDecisionSection1@1";
pub const CONTENT_TYPE: &str = "application/vnd.aurora.certified-decision-section-1+json";
pub const PRECLINICAL_BOUNDARY: &str = "preclinical-research-only; no human-subject or clinical-source data; no diagnosis, treatment, triage, enrollment, or clinical decisions";
pub const MAX_FACTS: usize = 16_384;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContextEvidenceState {
    Proven,
    Supported,
    Unknown,
    Unmeasured,
    Contradicted,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DecisionQuery4 {
    pub request_id: String,
    pub federation_id: String,
    pub requester: String,
    pub purpose: String,
    pub semantic_profile: String,
    pub required_claims: Vec<String>,
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
pub struct ContextFact4 {
    pub fact_id: String,
    pub source_id: String,
    pub origin: String,
    pub claim: String,
    pub influence_milli: i64,
    pub freshness_milli: i64,
    pub semantic_profile: String,
    pub terms: Vec<String>,
    pub content_digest: ContentHash,
    pub provenance_digest: ContentHash,
    pub replay_identity: ContentHash,
    pub evidence_state: ContextEvidenceState,
    pub signed: bool,
    pub permitted: bool,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub negative_result: bool,
    pub omission_reasons: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContextPeer4 {
    pub peer_id: String,
    pub origin: String,
    pub federation_id: String,
    pub semantic_profile: String,
    pub checkpoint: u64,
    pub context_digest: ContentHash,
    pub source_count: usize,
    pub evidence_state: ContextEvidenceState,
    pub signed: bool,
    pub aggregate_only: bool,
    pub raw_data_local: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CertifiedDecisionSection1Artifact {
    pub artifact_id: String,
    pub content_type: String,
    pub content_hash: ContentHash,
    pub semantic_loss: Vec<String>,
    pub provenance_digests: Vec<ContentHash>,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CertifiedDecisionSection1 {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub federation_id: String,
    pub requester: String,
    pub purpose: String,
    pub semantic_profile: String,
    pub checkpoint: u64,
    pub disposition: String,
    pub candidate_order: Vec<String>,
    pub selected_order: Vec<String>,
    pub unresolved_order: Vec<String>,
    pub blocked_order: Vec<String>,
    pub source_order: Vec<String>,
    pub selected_source_order: Vec<String>,
    pub missing_source_order: Vec<String>,
    pub peer_order: Vec<String>,
    pub qualified_peer_order: Vec<String>,
    pub missing_peer_order: Vec<String>,
    pub omission_order: Vec<String>,
    pub uncertainty_order: Vec<String>,
    pub negative_evidence_order: Vec<String>,
    pub influence_scores_milli: Vec<i64>,
    pub replay_identity: ContentHash,
    pub section_digest: ContentHash,
    pub artifact: CertifiedDecisionSection1Artifact,
    pub effect_receipts: Vec<String>,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub boundary: String,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ContextCompilationError {
    #[error("invalid context compilation request: {0}")]
    Invalid(String),
    #[error("context compilation artifact failed: {0}")]
    Artifact(String),
}

pub fn context_compilation_manifest() -> serde_json::Value {
    json!({
        "schema_version":"aurora-research-contract/1.0", "capability_id":FEATURE_ID,
        "version":CONTRACT_VERSION, "owner_crate":"ids",
        "consumers":["formal methods researcher","computational biologist","federation steward"],
        "behavior":"compiles bounded typed decision facts and peer context attestations into an omission-aware certified section",
        "value":"makes federated context selection deterministic, replayable, and fail-closed without exporting raw research data",
        "input_schema":INPUT_SCHEMA, "output_schema":OUTPUT_SCHEMA,
        "effects":["exchange:permitted-context-summaries","manage:local-capability"],
        "permissions":["read:local-research-artifacts","operate:institution-node"], "autonomy_tier":"A1",
        "boundary":PRECLINICAL_BOUNDARY
    })
}

impl CertifiedDecisionSection1 {
    pub fn validate(&self) -> Result<(), ContextCompilationError> {
        if self.schema_version != "aurora-research-contract/1.0"
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.boundary != PRECLINICAL_BOUNDARY
            || !self.raw_data_local
            || !self.aggregate_only
            || self.request_id.trim().is_empty()
            || self.federation_id.trim().is_empty()
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
            return Err(ContextCompilationError::Invalid("context identity, checkpoint, locality, candidates, sources, peers, or effects are incomplete".into()));
        }
        for values in [
            &self.candidate_order,
            &self.selected_order,
            &self.unresolved_order,
            &self.blocked_order,
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
                return Err(ContextCompilationError::Invalid(
                    "context ordering is not canonical".into(),
                ));
            }
        }
        let candidates = BTreeSet::from_iter(self.candidate_order.iter().cloned());
        let parts = self
            .selected_order
            .iter()
            .chain(&self.unresolved_order)
            .chain(&self.blocked_order)
            .cloned()
            .collect::<BTreeSet<_>>();
        if candidates != parts || candidates.len() != self.candidate_order.len() {
            return Err(ContextCompilationError::Invalid(
                "candidate dispositions do not partition".into(),
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
            return Err(ContextCompilationError::Invalid(
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
            return Err(ContextCompilationError::Invalid(
                "peer dispositions do not partition".into(),
            ));
        }
        if self.selected_order.len() != self.influence_scores_milli.len()
            || self.artifact.content_type != CONTENT_TYPE
            || self.artifact.boundary != PRECLINICAL_BOUNDARY
            || self.artifact.content_hash != self.section_digest
        {
            return Err(ContextCompilationError::Artifact(
                "artifact metadata, score cardinality, or digest is inconsistent".into(),
            ));
        }
        if self.effect_receipts.iter().any(|effect| {
            !effect.starts_with("exchange:permitted-context-summaries:")
                && !effect.starts_with("manage:local-capability:")
                && effect != "block:unsafe-release"
        }) {
            return Err(ContextCompilationError::Invalid(
                "effect is outside context compilation gate".into(),
            ));
        }
        Ok(())
    }
    pub fn digest(&self) -> Result<ContentHash, ContextCompilationError> {
        self.validate()?;
        ContentHash::of_value(
            &serde_json::to_value(self)
                .map_err(|e| ContextCompilationError::Artifact(e.to_string()))?,
        )
        .map_err(|e| ContextCompilationError::Artifact(e.to_string()))
    }
}

pub fn operate_context_compilation(
    request: &DecisionQuery4,
    facts: &[ContextFact4],
    peers: &[ContextPeer4],
) -> Result<CertifiedDecisionSection1, ContextCompilationError> {
    validate_request(request, facts, peers)?;
    let mut rows = facts.to_vec();
    rows.sort_by(|a, b| {
        b.influence_milli
            .cmp(&a.influence_milli)
            .then(b.freshness_milli.cmp(&a.freshness_milli))
            .then(a.fact_id.cmp(&b.fact_id))
    });
    let candidate_order = rows.iter().map(|x| x.fact_id.clone()).collect::<Vec<_>>();
    let mut peer_rows = peers.to_vec();
    peer_rows.sort_by(|a, b| a.peer_id.cmp(&b.peer_id));
    let peer_order = peer_rows
        .iter()
        .map(|x| x.peer_id.clone())
        .collect::<Vec<_>>();
    let mut qualified_peers = BTreeSet::new();
    let mut missing_peers = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    for peer in &peer_rows {
        let ok = peer.federation_id == request.federation_id
            && peer.semantic_profile == request.semantic_profile
            && peer.checkpoint == request.checkpoint
            && peer.source_count >= request.minimum_source_quorum
            && peer.signed
            && peer.aggregate_only
            && peer.raw_data_local
            && matches!(
                peer.evidence_state,
                ContextEvidenceState::Proven | ContextEvidenceState::Supported
            );
        if ok {
            qualified_peers.insert(peer.peer_id.clone());
        } else {
            missing_peers.insert(peer.peer_id.clone());
            uncertainty.insert(format!("peer:{}:not-qualified", peer.peer_id));
        }
    }
    let mut selected = BTreeSet::new();
    let mut unresolved = BTreeSet::new();
    let mut blocked = BTreeSet::new();
    let mut selected_sources = BTreeSet::new();
    let mut omissions = BTreeSet::new();
    let mut negative = BTreeSet::new();
    let mut scores = Vec::new();
    for row in &rows {
        if row.negative_result {
            negative.insert(format!("{}:negative-result", row.fact_id));
        }
        for reason in &row.omission_reasons {
            omissions.insert(format!("{}:{}", row.fact_id, reason));
        }
        let mut reasons = Vec::new();
        if row.semantic_profile != request.semantic_profile {
            reasons.push("semantic-profile-mismatch");
        }
        let missing = request
            .required_claims
            .iter()
            .filter(|term| !row.terms.contains(term))
            .count();
        if missing > 0 {
            reasons.push("required-claim-missing");
            omissions.insert(format!("{}:missing-claims:{}", row.fact_id, missing));
        }
        if row.replay_identity != request.replay_identity {
            reasons.push("replay-identity-mismatch");
        }
        if !row.signed || !row.permitted {
            reasons.push("authorization-missing");
        }
        if !row.raw_data_local || !row.aggregate_only {
            reasons.push("locality-or-aggregate-only-failed");
        }
        if row.evidence_state == ContextEvidenceState::Contradicted {
            blocked.insert(row.fact_id.clone());
            negative.insert(format!("{}:contradicted", row.fact_id));
        } else if !matches!(
            row.evidence_state,
            ContextEvidenceState::Proven | ContextEvidenceState::Supported
        ) || !reasons.is_empty()
        {
            unresolved.insert(row.fact_id.clone());
            uncertainty.insert(format!("{}:unresolved", row.fact_id));
        } else {
            selected.insert(row.fact_id.clone());
            selected_sources.insert(row.source_id.clone());
            scores.push(row.influence_milli.saturating_add(row.freshness_milli));
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
    if selected_sources.len() < request.minimum_source_quorum {
        uncertainty.insert("source:minimum-quorum-unmet".into());
    }
    let disposition = if global_block || !blocked.is_empty() {
        "blocked"
    } else if selected.is_empty()
        || selected_sources.len() < request.minimum_source_quorum
        || qualified_peers.len() < request.minimum_peer_quorum
    {
        "unresolved"
    } else {
        "qualified"
    };
    if global_block {
        blocked.extend(candidate_order.iter().cloned());
        selected.clear();
        unresolved.clear();
        scores.clear();
    }
    if disposition != "qualified" {
        omissions.insert("request:context-gates-incomplete".into());
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
    let selected_source_order = selected_sources.into_iter().collect::<Vec<_>>();
    let missing_source_order = source_order
        .iter()
        .filter(|x| !selected_source_order.contains(x))
        .cloned()
        .collect::<Vec<_>>();
    let payload = json!({"schema_version":"aurora-research-contract/1.0","contract_version":CONTRACT_VERSION,"feature_id":FEATURE_ID,"request_id":request.request_id,"federation_id":request.federation_id,"requester":request.requester,"purpose":request.purpose,"semantic_profile":request.semantic_profile,"checkpoint":request.checkpoint,"disposition":disposition,"candidate_order":candidate_order,"selected_order":selected_order,"unresolved_order":unresolved_order,"blocked_order":blocked_order,"source_order":source_order,"selected_source_order":selected_source_order,"missing_source_order":missing_source_order,"peer_order":peer_order,"qualified_peer_order":qualified_peers,"missing_peer_order":missing_peers,"omission_order":omissions,"uncertainty_order":uncertainty,"negative_evidence_order":negative,"influence_scores_milli":scores,"replay_identity":request.replay_identity,"boundary":PRECLINICAL_BOUNDARY});
    let section_digest = ContentHash::of_value(&payload)
        .map_err(|e| ContextCompilationError::Artifact(e.to_string()))?;
    let artifact = CertifiedDecisionSection1Artifact {
        artifact_id: format!("certified-decision-section-1:{}", request.request_id),
        content_type: CONTENT_TYPE.into(),
        content_hash: section_digest.clone(),
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
            format!(
                "exchange:permitted-context-summaries:{}",
                request.request_id
            ),
            format!("manage:local-capability:{}", request.request_id),
        ]
    } else {
        vec!["block:unsafe-release".into()]
    };
    let receipt = CertifiedDecisionSection1 {
        schema_version: "aurora-research-contract/1.0".into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        request_id: request.request_id.clone(),
        federation_id: request.federation_id.clone(),
        requester: request.requester.clone(),
        purpose: request.purpose.clone(),
        semantic_profile: request.semantic_profile.clone(),
        checkpoint: request.checkpoint,
        disposition: disposition.into(),
        candidate_order: payload["candidate_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|x| x.as_str().unwrap().into())
            .collect(),
        selected_order: payload["selected_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|x| x.as_str().unwrap().into())
            .collect(),
        unresolved_order: payload["unresolved_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|x| x.as_str().unwrap().into())
            .collect(),
        blocked_order: payload["blocked_order"]
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
        influence_scores_milli: scores,
        replay_identity: request.replay_identity.clone(),
        section_digest,
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
    request: &DecisionQuery4,
    facts: &[ContextFact4],
    peers: &[ContextPeer4],
) -> Result<(), ContextCompilationError> {
    if request.request_id.trim().is_empty()
        || request.federation_id.trim().is_empty()
        || request.requester.trim().is_empty()
        || request.purpose.trim().is_empty()
        || request.semantic_profile.trim().is_empty()
        || request.required_claims.is_empty()
        || request.candidate_limit == 0
        || request.candidate_limit > MAX_FACTS
        || request.minimum_source_quorum == 0
        || request.minimum_peer_quorum == 0
        || request.budget_units == 0
        || request.checkpoint == 0
        || request.replay_identity.as_str().len() != 64
        || request.boundary != PRECLINICAL_BOUNDARY
        || !request.raw_data_local
        || !request.aggregate_only
        || facts.is_empty()
        || facts.len() > MAX_FACTS
        || peers.is_empty()
    {
        return Err(ContextCompilationError::Invalid("request identity, claims, bounds, replay, locality, facts, peers, or boundary is invalid".into()));
    }
    let mut ids = BTreeSet::new();
    for fact in facts {
        if fact.fact_id.trim().is_empty()
            || fact.source_id.trim().is_empty()
            || fact.origin.trim().is_empty()
            || fact.claim.trim().is_empty()
            || fact.content_digest.as_str().len() != 64
            || fact.provenance_digest.as_str().len() != 64
            || fact.replay_identity.as_str().len() != 64
            || !ids.insert(fact.fact_id.clone())
        {
            return Err(ContextCompilationError::Invalid(
                "fact identity, uniqueness, origin, claim, or digest is invalid".into(),
            ));
        }
    }
    let mut peer_ids = BTreeSet::new();
    for peer in peers {
        if peer.peer_id.trim().is_empty()
            || peer.origin.trim().is_empty()
            || peer.context_digest.as_str().len() != 64
            || !peer_ids.insert(peer.peer_id.clone())
        {
            return Err(ContextCompilationError::Invalid(
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
    fn request() -> DecisionQuery4 {
        DecisionQuery4 {
            request_id: "request:context".into(),
            federation_id: "federation:1".into(),
            requester: "formal-methods".into(),
            purpose: "decision-context".into(),
            semantic_profile: "neuro:v1".into(),
            required_claims: vec!["neuron".into()],
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
    fn fact(id: &str, state: ContextEvidenceState) -> ContextFact4 {
        ContextFact4 {
            fact_id: id.into(),
            source_id: format!("source:{id}"),
            origin: "site-a".into(),
            claim: "neuron claim".into(),
            influence_milli: 90,
            freshness_milli: 80,
            semantic_profile: "neuro:v1".into(),
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
    fn peer() -> ContextPeer4 {
        ContextPeer4 {
            peer_id: "peer:a".into(),
            origin: "site-a".into(),
            federation_id: "federation:1".into(),
            semantic_profile: "neuro:v1".into(),
            checkpoint: 1,
            context_digest: h("peer"),
            source_count: 1,
            evidence_state: ContextEvidenceState::Supported,
            signed: true,
            aggregate_only: true,
            raw_data_local: true,
        }
    }
    #[test]
    fn manifest_is_a1() {
        assert_eq!(context_compilation_manifest()["autonomy_tier"], "A1");
    }
    #[test]
    fn qualified_is_replayable() {
        let r = operate_context_compilation(
            &request(),
            &[
                fact("b", ContextEvidenceState::Supported),
                fact("a", ContextEvidenceState::Proven),
            ],
            &[peer()],
        )
        .unwrap();
        assert_eq!(r.disposition, "qualified");
        assert_eq!(r.digest().unwrap(), r.digest().unwrap());
    }
    #[test]
    fn unknown_is_unresolved() {
        let r = operate_context_compilation(
            &request(),
            &[fact("a", ContextEvidenceState::Unknown)],
            &[peer()],
        )
        .unwrap();
        assert_eq!(r.disposition, "unresolved");
        assert!(!r.uncertainty_order.is_empty());
    }
    #[test]
    fn contradiction_blocks() {
        let r = operate_context_compilation(
            &request(),
            &[fact("a", ContextEvidenceState::Contradicted)],
            &[peer()],
        )
        .unwrap();
        assert_eq!(r.disposition, "blocked");
        assert!(!r.negative_evidence_order.is_empty());
    }
    #[test]
    fn policy_denial_blocks() {
        let mut q = request();
        q.policy_allow = false;
        let r = operate_context_compilation(
            &q,
            &[fact("a", ContextEvidenceState::Supported)],
            &[peer()],
        )
        .unwrap();
        assert_eq!(r.disposition, "blocked");
    }
    #[test]
    fn duplicate_is_rejected() {
        assert!(operate_context_compilation(
            &request(),
            &[
                fact("a", ContextEvidenceState::Supported),
                fact("a", ContextEvidenceState::Supported)
            ],
            &[peer()]
        )
        .is_err());
    }
}
