//! Prospective high-throughput context-compilation assurance for `AFA-api-P03-F27`.
//!
//! This API contract turns scoped facts and peer attestations into a deterministic omission
//! certificate. It never retrieves sources or exports raw experimental data.

use bioprism_foundation::{EvidenceState, PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-api-P03-F27";
pub const CONTRACT_VERSION: &str =
    "api-prospective-high-throughput-context-compilation-assurance/1.0";
pub const INPUT_SCHEMA: &str = "ContextCompilationRequest6@1";
pub const OUTPUT_SCHEMA: &str = "ContextAssuranceReceipt7@1";
pub const CONTENT_TYPE: &str = "application/vnd.aurora.context-assurance-receipt-7+json";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContextFact6 {
    pub fact_id: String,
    pub scope: String,
    pub semantic_profile: String,
    pub influence_milli: u32,
    pub evidence_state: EvidenceState,
    pub source_digest: ContentHash,
    pub provenance_digest: ContentHash,
    pub replay_identity: ContentHash,
    pub local_data: bool,
    pub policy_allowed: bool,
    pub negative_result: bool,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContextPeer5 {
    pub peer_id: String,
    pub origin: String,
    pub semantic_profile: String,
    pub checkpoint: u64,
    pub context_digest: ContentHash,
    pub evidence_state: EvidenceState,
    pub signed: bool,
    pub aggregate_only: bool,
    pub raw_data_local: bool,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContextCompilationRequest6 {
    pub request_id: String,
    pub federation_id: String,
    pub query_id: String,
    pub requester: String,
    pub purpose: String,
    pub scope: String,
    pub semantic_profile: String,
    pub required_fact_order: Vec<String>,
    pub facts: Vec<ContextFact6>,
    pub peers: Vec<ContextPeer5>,
    pub checkpoint: u64,
    pub minimum_peer_quorum: usize,
    pub policy_allow: bool,
    pub protected_closure: bool,
    pub signed_approval: bool,
    pub federation_approved: bool,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub replay_identity: ContentHash,
    pub boundary: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContextAssuranceArtifact7 {
    pub artifact_id: String,
    pub content_type: String,
    pub content_hash: ContentHash,
    pub semantic_loss: Vec<String>,
    pub provenance_digests: Vec<ContentHash>,
    pub boundary: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContextAssuranceReceipt7 {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub federation_id: String,
    pub query_id: String,
    pub requester: String,
    pub purpose: String,
    pub scope: String,
    pub semantic_profile: String,
    pub checkpoint: u64,
    pub disposition: String,
    pub fact_order: Vec<String>,
    pub selected_fact_order: Vec<String>,
    pub omitted_fact_order: Vec<String>,
    pub unresolved_fact_order: Vec<String>,
    pub blocked_fact_order: Vec<String>,
    pub missing_fact_order: Vec<String>,
    pub peer_order: Vec<String>,
    pub qualified_peer_order: Vec<String>,
    pub missing_peer_order: Vec<String>,
    pub omission_order: Vec<String>,
    pub uncertainty_order: Vec<String>,
    pub negative_evidence_order: Vec<String>,
    pub replay_identity: ContentHash,
    pub context_digest: ContentHash,
    pub artifact: ContextAssuranceArtifact7,
    pub effect_receipts: Vec<String>,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub boundary: String,
}
#[derive(Debug, Error, PartialEq, Eq)]
pub enum ContextAssuranceError {
    #[error("invalid context assurance request: {0}")]
    Invalid(String),
    #[error("context assurance artifact failed: {0}")]
    Artifact(String),
}
pub fn context_compilation_assurance_manifest() -> serde_json::Value {
    json!({"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"capability_id":FEATURE_ID,"version":CONTRACT_VERSION,"owner_crate":"api","consumers":["context researcher","API client","federation steward"],"behavior":"compiles scoped typed facts into a deterministic context readiness receipt with omission certificate","value":"prevents incomplete or unsupported context from being presented as decision-sufficient","input_schema":INPUT_SCHEMA,"output_schema":OUTPUT_SCHEMA,"effects":["retain:context-assurance","exchange:aggregate-context-summary"],"permissions":["retain:context-receipts","exchange:aggregate-context"],"autonomy_tier":"A1","boundary":PRECLINICAL_BOUNDARY})
}
impl ContextAssuranceReceipt7 {
    pub fn validate(&self) -> Result<(), ContextAssuranceError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.boundary != PRECLINICAL_BOUNDARY
            || self.artifact.boundary != PRECLINICAL_BOUNDARY
            || !self.raw_data_local
            || !self.aggregate_only
            || self.checkpoint == 0
            || self.request_id.trim().is_empty()
            || self.federation_id.trim().is_empty()
            || self.query_id.trim().is_empty()
            || self.requester.trim().is_empty()
            || self.purpose.trim().is_empty()
            || self.scope.trim().is_empty()
            || self.semantic_profile.trim().is_empty()
            || self.fact_order.is_empty()
            || self.peer_order.is_empty()
            || self.effect_receipts.is_empty()
        {
            return Err(ContextAssuranceError::Invalid(
                "context identity, checkpoint, locality, facts, peers, or effects are incomplete"
                    .into(),
            ));
        }
        for v in [
            &self.fact_order,
            &self.selected_fact_order,
            &self.omitted_fact_order,
            &self.unresolved_fact_order,
            &self.blocked_fact_order,
            &self.missing_fact_order,
            &self.peer_order,
            &self.qualified_peer_order,
            &self.missing_peer_order,
            &self.omission_order,
            &self.uncertainty_order,
            &self.negative_evidence_order,
            &self.effect_receipts,
        ] {
            if v.windows(2).any(|w| w[0] >= w[1]) {
                return Err(ContextAssuranceError::Invalid(
                    "context ordering is not canonical".into(),
                ));
            }
        }
        let all = BTreeSet::from_iter(self.fact_order.iter().cloned());
        let parts = self
            .selected_fact_order
            .iter()
            .chain(&self.omitted_fact_order)
            .chain(&self.unresolved_fact_order)
            .chain(&self.blocked_fact_order)
            .chain(&self.missing_fact_order)
            .cloned()
            .collect::<BTreeSet<_>>();
        if all != parts || all.len() != self.fact_order.len() {
            return Err(ContextAssuranceError::Invalid(
                "context facts do not partition".into(),
            ));
        }
        let peers = BTreeSet::from_iter(self.peer_order.iter().cloned());
        let pp = self
            .qualified_peer_order
            .iter()
            .chain(&self.missing_peer_order)
            .cloned()
            .collect::<BTreeSet<_>>();
        if peers != pp || peers.len() != self.peer_order.len() {
            return Err(ContextAssuranceError::Invalid(
                "context peers do not partition".into(),
            ));
        }
        if self.artifact.content_type != CONTENT_TYPE
            || self.artifact.content_hash != self.context_digest
        {
            return Err(ContextAssuranceError::Artifact(
                "context artifact digest is inconsistent".into(),
            ));
        }
        Ok(())
    }
}
pub fn assure_context_compilation(
    q: &ContextCompilationRequest6,
) -> Result<ContextAssuranceReceipt7, ContextAssuranceError> {
    validate_request(q)?;
    let mut facts = q.facts.clone();
    facts.sort_by(|a, b| {
        b.influence_milli
            .cmp(&a.influence_milli)
            .then(a.fact_id.cmp(&b.fact_id))
    });
    let required = q
        .required_fact_order
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let mut fact_order = facts.iter().map(|f| f.fact_id.clone()).collect::<Vec<_>>();
    for required_id in &required {
        if !fact_order.iter().any(|id| id == required_id) {
            fact_order.push(required_id.clone());
        }
    }
    fact_order.sort();
    let mut selected = BTreeSet::new();
    let mut omitted = BTreeSet::new();
    let mut unresolved = BTreeSet::new();
    let mut blocked = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    let mut negative = BTreeSet::new();
    let mut seen = BTreeSet::new();
    for f in &facts {
        seen.insert(f.fact_id.clone());
        if f.negative_result {
            negative.insert(format!("{}:negative-result", f.fact_id));
        }
        match f.evidence_state {
            EvidenceState::Contradicted => {
                blocked.insert(f.fact_id.clone());
                negative.insert(format!("{}:contradicted", f.fact_id));
            }
            EvidenceState::Unknown | EvidenceState::Speculative => {
                unresolved.insert(f.fact_id.clone());
                uncertainty.insert(format!("{}:evidence-state", f.fact_id));
            }
            EvidenceState::Proven | EvidenceState::Supported
                if f.scope == q.scope
                    && f.semantic_profile == q.semantic_profile
                    && f.local_data
                    && f.policy_allowed =>
            {
                selected.insert(f.fact_id.clone());
            }
            EvidenceState::Proven | EvidenceState::Supported => {
                omitted.insert(f.fact_id.clone());
                uncertainty.insert(format!("{}:scope-profile-locality", f.fact_id));
            }
        }
    }
    let missing = required.difference(&seen).cloned().collect::<BTreeSet<_>>();
    let mut omissions = missing
        .iter()
        .map(|x| format!("fact:{}:missing", x))
        .collect::<BTreeSet<_>>();
    for x in &omitted {
        omissions.insert(format!("fact:{}:omitted", x));
    }
    let mut peers = q.peers.clone();
    peers.sort_by(|a, b| a.peer_id.cmp(&b.peer_id));
    let peer_order = peers.iter().map(|p| p.peer_id.clone()).collect::<Vec<_>>();
    let qualified_peers = peers
        .iter()
        .filter(|p| {
            p.semantic_profile == q.semantic_profile
                && p.checkpoint == q.checkpoint
                && p.signed
                && p.aggregate_only
                && p.raw_data_local
                && matches!(
                    p.evidence_state,
                    EvidenceState::Proven | EvidenceState::Supported
                )
        })
        .map(|p| p.peer_id.clone())
        .collect::<BTreeSet<_>>();
    let missing_peers = peer_order
        .iter()
        .filter(|p| !qualified_peers.contains(*p))
        .cloned()
        .collect::<BTreeSet<_>>();
    for p in &missing_peers {
        uncertainty.insert(format!("peer:{}:not-qualified", p));
    }
    let global = !q.policy_allow
        || !q.protected_closure
        || !q.signed_approval
        || !q.federation_approved
        || !q.raw_data_local
        || !q.aggregate_only;
    if !q.policy_allow {
        negative.insert("request:policy-denied".into());
    }
    if !q.protected_closure {
        uncertainty.insert("request:protected-closure-incomplete".into());
    }
    if !q.signed_approval {
        uncertainty.insert("request:signed-approval-missing".into());
    }
    if !q.federation_approved {
        uncertainty.insert("request:federation-approval-missing".into());
    }
    let disposition = if global || !blocked.is_empty() {
        "blocked"
    } else if selected.is_empty()
        || !missing.is_empty()
        || !unresolved.is_empty()
        || qualified_peers.len() < q.minimum_peer_quorum
    {
        "unresolved"
    } else {
        "qualified"
    };
    if disposition != "qualified" {
        omissions.insert("request:context-not-release-ready".into());
    }
    let payload = json!({"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"contract_version":CONTRACT_VERSION,"feature_id":FEATURE_ID,"request_id":q.request_id,"federation_id":q.federation_id,"query_id":q.query_id,"requester":q.requester,"purpose":q.purpose,"scope":q.scope,"semantic_profile":q.semantic_profile,"checkpoint":q.checkpoint,"disposition":disposition,"fact_order":fact_order,"selected_fact_order":selected,"omitted_fact_order":omitted,"unresolved_fact_order":unresolved,"blocked_fact_order":blocked,"missing_fact_order":missing,"peer_order":peer_order,"qualified_peer_order":qualified_peers,"missing_peer_order":missing_peers,"omission_order":omissions,"uncertainty_order":uncertainty,"negative_evidence_order":negative,"replay_identity":q.replay_identity,"boundary":PRECLINICAL_BOUNDARY});
    let digest = ContentHash::of_value(&payload)
        .map_err(|e| ContextAssuranceError::Artifact(e.to_string()))?;
    let artifact = ContextAssuranceArtifact7 {
        artifact_id: format!("context-assurance-receipt-7:{}", q.request_id),
        content_type: CONTENT_TYPE.into(),
        content_hash: digest.clone(),
        semantic_loss: Vec::new(),
        provenance_digests: facts
            .iter()
            .map(|f| f.provenance_digest.clone())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect(),
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    let effects = if disposition == "qualified" {
        vec![
            format!("retain:context-assurance:{}", q.request_id),
            format!("exchange:aggregate-context-summary:{}", q.request_id),
        ]
    } else {
        vec!["block:unsafe-release".into()]
    };
    let r = ContextAssuranceReceipt7 {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        request_id: q.request_id.clone(),
        federation_id: q.federation_id.clone(),
        query_id: q.query_id.clone(),
        requester: q.requester.clone(),
        purpose: q.purpose.clone(),
        scope: q.scope.clone(),
        semantic_profile: q.semantic_profile.clone(),
        checkpoint: q.checkpoint,
        disposition: disposition.into(),
        fact_order,
        selected_fact_order: selected.into_iter().collect(),
        omitted_fact_order: omitted.into_iter().collect(),
        unresolved_fact_order: unresolved.into_iter().collect(),
        blocked_fact_order: blocked.into_iter().collect(),
        missing_fact_order: missing.into_iter().collect(),
        peer_order,
        qualified_peer_order: qualified_peers.into_iter().collect(),
        missing_peer_order: missing_peers.into_iter().collect(),
        omission_order: omissions.into_iter().collect(),
        uncertainty_order: uncertainty.into_iter().collect(),
        negative_evidence_order: negative.into_iter().collect(),
        replay_identity: q.replay_identity.clone(),
        context_digest: digest,
        artifact,
        effect_receipts: effects,
        raw_data_local: q.raw_data_local,
        aggregate_only: q.aggregate_only,
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    r.validate()?;
    Ok(r)
}
fn validate_request(q: &ContextCompilationRequest6) -> Result<(), ContextAssuranceError> {
    if ![
        &q.request_id,
        &q.federation_id,
        &q.query_id,
        &q.requester,
        &q.purpose,
        &q.scope,
        &q.semantic_profile,
    ]
    .iter()
    .all(|v| !v.trim().is_empty())
        || q.required_fact_order.is_empty()
        || q.facts.is_empty()
        || q.peers.is_empty()
        || q.checkpoint == 0
        || q.minimum_peer_quorum == 0
        || !q.raw_data_local
        || !q.aggregate_only
        || q.boundary != PRECLINICAL_BOUNDARY
        || q.replay_identity.as_str().len() != 64
    {
        return Err(ContextAssuranceError::Invalid(
            "context identity, bounds, facts, peers, replay, locality, or boundary is invalid"
                .into(),
        ));
    }
    let mut ids = BTreeSet::new();
    for f in &q.facts {
        if f.fact_id.trim().is_empty()
            || !ids.insert(f.fact_id.clone())
            || f.scope.trim().is_empty()
            || f.semantic_profile.trim().is_empty()
            || f.source_digest.as_str().len() != 64
            || f.provenance_digest.as_str().len() != 64
            || f.replay_identity != q.replay_identity
        {
            return Err(ContextAssuranceError::Invalid(
                "fact identity, scope, profile, digests, or replay is invalid".into(),
            ));
        }
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    fn h(v: &str) -> ContentHash {
        ContentHash::of_bytes(v.as_bytes())
    }
    fn q() -> ContextCompilationRequest6 {
        let r = h("replay");
        ContextCompilationRequest6 {
            request_id: "request:context".into(),
            federation_id: "fed:context".into(),
            query_id: "query:1".into(),
            requester: "researcher".into(),
            purpose: "decision-context".into(),
            scope: "organoid".into(),
            semantic_profile: "neuro:v1".into(),
            required_fact_order: vec!["fact:a".into()],
            facts: vec![ContextFact6 {
                fact_id: "fact:a".into(),
                scope: "organoid".into(),
                semantic_profile: "neuro:v1".into(),
                influence_milli: 900,
                evidence_state: EvidenceState::Supported,
                source_digest: h("source"),
                provenance_digest: h("prov"),
                replay_identity: r.clone(),
                local_data: true,
                policy_allowed: true,
                negative_result: false,
            }],
            peers: vec![ContextPeer5 {
                peer_id: "peer:a".into(),
                origin: "site:a".into(),
                semantic_profile: "neuro:v1".into(),
                checkpoint: 2,
                context_digest: h("ctx"),
                evidence_state: EvidenceState::Supported,
                signed: true,
                aggregate_only: true,
                raw_data_local: true,
            }],
            checkpoint: 2,
            minimum_peer_quorum: 1,
            policy_allow: true,
            protected_closure: true,
            signed_approval: true,
            federation_approved: true,
            raw_data_local: true,
            aggregate_only: true,
            replay_identity: r,
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }
    #[test]
    fn qualified_context() {
        assert_eq!(
            assure_context_compilation(&q()).unwrap().disposition,
            "qualified"
        )
    }
    #[test]
    fn unknown_unresolved() {
        let mut x = q();
        x.facts[0].evidence_state = EvidenceState::Unknown;
        assert_eq!(
            assure_context_compilation(&x).unwrap().disposition,
            "unresolved"
        )
    }
}
