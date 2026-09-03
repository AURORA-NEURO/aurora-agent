//! Federated continual knowledge-representation assurance (`AFA-obligation-P04-F28`).
//!
//! This module is a read-only verifier for typed claim and peer summaries.  It never imports
//! raw experimental data or opens federation connections; instead it emits a deterministic,
//! content-addressed `TypedKnowledgeWorld7` receipt that keeps incomplete closure visible.

use bioprism_foundation::{
    AuthorityRequirement, AutonomyTier, CapabilityManifest, Determinism, Effect, EvidenceReference,
    ResearchSurface, TypedPort, PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION,
};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-obligation-P04-F28";
pub const CONTRACT_VERSION: &str = "obligation-federated-knowledge-representation-assurance/1.0";
pub const INPUT_SCHEMA: &str = "ScopedResearchClaims4@1";
pub const OUTPUT_SCHEMA: &str = "TypedKnowledgeWorld7@1";
pub const CONTENT_TYPE: &str = "application/vnd.aurora.typed-knowledge-world-7+json";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum KnowledgeEvidenceState {
    Proven,
    Supported,
    Unknown,
    Unmeasured,
    Contradicted,
    Negative,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScopedResearchClaims4 {
    pub schema_version: String,
    pub request_id: String,
    pub world_id: String,
    pub federation_id: String,
    pub requester: String,
    pub purpose: String,
    pub semantic_profile: String,
    pub checkpoint: u64,
    pub required_claim_order: Vec<String>,
    pub required_source_order: Vec<String>,
    pub required_peer_order: Vec<String>,
    pub replay_identity: ContentHash,
    pub policy_allow: bool,
    pub protected_closure: bool,
    pub federation_allow: bool,
    pub signed_approval: bool,
    pub aggregate_only: bool,
    pub raw_data_local: bool,
    pub budget_units: u64,
    pub adversarial_event_order: Vec<String>,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResearchClaim4 {
    pub claim_id: String,
    pub subject: String,
    pub predicate: String,
    pub object: String,
    pub source_id: String,
    pub study_id: String,
    pub modality: String,
    pub semantic_profile: String,
    pub confidence_milli: u16,
    pub evidence_state: KnowledgeEvidenceState,
    pub content_digest: ContentHash,
    pub provenance_digest: ContentHash,
    pub replay_identity: ContentHash,
    pub signed: bool,
    pub permitted: bool,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub omission_order: Vec<String>,
    pub uncertainty_order: Vec<String>,
    pub negative_result: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KnowledgePeer4 {
    pub peer_id: String,
    pub source_order: Vec<String>,
    pub world_digest: ContentHash,
    pub semantic_profile: String,
    pub checkpoint: u64,
    pub evidence_state: KnowledgeEvidenceState,
    pub signed: bool,
    pub permitted: bool,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub replay_identity: ContentHash,
    pub omission_order: Vec<String>,
    pub uncertainty_order: Vec<String>,
    pub negative_result: bool,
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
    pub missing_claim_order: Vec<String>,
    pub source_order: Vec<String>,
    pub selected_source_order: Vec<String>,
    pub missing_source_order: Vec<String>,
    pub peer_order: Vec<String>,
    pub qualified_peer_order: Vec<String>,
    pub missing_peer_order: Vec<String>,
    pub omission_order: Vec<String>,
    pub uncertainty_order: Vec<String>,
    pub negative_evidence_order: Vec<String>,
    pub confidence_scores_milli: Vec<u16>,
    pub replay_identity: ContentHash,
    pub world_digest: ContentHash,
    pub artifact: TypedKnowledgeWorld7Artifact,
    pub effect_receipts: Vec<String>,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub boundary: String,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum KnowledgeRepresentationAssuranceError {
    #[error("invalid knowledge representation assurance request: {0}")]
    Invalid(String),
    #[error("knowledge representation assurance artifact failed: {0}")]
    Artifact(String),
}

fn hash_ok(value: &ContentHash) -> bool {
    value.as_str().len() == 64 && value.as_str().bytes().all(|b| b.is_ascii_hexdigit())
}
fn ordered(values: &[String]) -> bool {
    values.windows(2).all(|w| w[0] < w[1])
}
fn digest(value: &Value) -> Result<ContentHash, KnowledgeRepresentationAssuranceError> {
    ContentHash::of_value(value)
        .map_err(|e| KnowledgeRepresentationAssuranceError::Artifact(e.to_string()))
}

pub fn knowledge_representation_assurance_manifest() -> CapabilityManifest {
    CapabilityManifest { schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(), capability_id: FEATURE_ID.into(), version: CONTRACT_VERSION.into(), owner_crate: "obligation".into(), consumers: ["computational biologist".into(), "federation verifier".into(), "knowledge curator".into()].into(), behavior: "verifies federated continual scoped claims and peer summaries into a deterministic typed knowledge-world receipt with omission-aware safety gates".into(), value: "prevents stale, contradictory, unauthorized, or incomplete knowledge from appearing as a qualified research world".into(), inputs: vec![TypedPort { name: "scoped_research_claims".into(), schema: INPUT_SCHEMA.into(), required: true }], outputs: vec![TypedPort { name: "typed_knowledge_world".into(), schema: OUTPUT_SCHEMA.into(), required: true }], effects: [Effect::ExecuteLocalComputation, Effect::WriteLocalArtifact].into(), permissions: ["evaluate:capability-runs".into()].into(), determinism: Determinism::ByteStable, evidence: vec![EvidenceReference { source_id: "slsa-provenance-1.2".into(), state: bioprism_foundation::EvidenceState::Supported, locator: Some("https://slsa.dev/spec/v1.2/provenance".into()) }], authority_requirements: vec![AuthorityRequirement { role: "federation knowledge steward".into(), reason: "federated knowledge verification is policy-bounded and never grants data-movement authority".into() }], autonomy_tier: AutonomyTier::A1, surfaces: [ResearchSurface::Ui, ResearchSurface::Api, ResearchSurface::Sdk, ResearchSurface::Cli, ResearchSurface::McpTool, ResearchSurface::Policy, ResearchSurface::Operator].into(), boundary: PRECLINICAL_BOUNDARY.into() }
}

impl TypedKnowledgeWorld7 {
    pub fn validate(&self) -> Result<(), KnowledgeRepresentationAssuranceError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
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
            || self.effect_receipts != vec!["block:unsafe-release"]
        {
            return Err(KnowledgeRepresentationAssuranceError::Invalid(
                "knowledge identity, closure, locality, or release gate is incomplete".into(),
            ));
        }
        for values in [
            &self.claim_order,
            &self.selected_claim_order,
            &self.unresolved_claim_order,
            &self.blocked_claim_order,
            &self.missing_claim_order,
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
            if !ordered(values) {
                return Err(KnowledgeRepresentationAssuranceError::Invalid(
                    "knowledge ordering is not canonical".into(),
                ));
            }
        }
        let ids = BTreeSet::from_iter(self.claim_order.iter().cloned());
        let parts = self
            .selected_claim_order
            .iter()
            .chain(&self.unresolved_claim_order)
            .chain(&self.blocked_claim_order)
            .chain(&self.missing_claim_order)
            .cloned()
            .collect::<BTreeSet<_>>();
        if ids.len() != self.claim_order.len() || parts != ids {
            return Err(KnowledgeRepresentationAssuranceError::Invalid(
                "claim outcomes do not partition".into(),
            ));
        }
        let sources = BTreeSet::from_iter(self.source_order.iter().cloned());
        let source_parts = self
            .selected_source_order
            .iter()
            .chain(&self.missing_source_order)
            .cloned()
            .collect::<BTreeSet<_>>();
        if sources.len() != self.source_order.len() || source_parts != sources {
            return Err(KnowledgeRepresentationAssuranceError::Invalid(
                "source outcomes do not partition".into(),
            ));
        }
        let peers = BTreeSet::from_iter(self.peer_order.iter().cloned());
        let peer_parts = self
            .qualified_peer_order
            .iter()
            .chain(&self.missing_peer_order)
            .cloned()
            .collect::<BTreeSet<_>>();
        if peers.len() != self.peer_order.len() || peer_parts != peers {
            return Err(KnowledgeRepresentationAssuranceError::Invalid(
                "peer outcomes do not partition".into(),
            ));
        }
        if self.selected_claim_order.len() != self.confidence_scores_milli.len()
            || self.artifact.content_type != CONTENT_TYPE
            || self.artifact.boundary != PRECLINICAL_BOUNDARY
            || self.artifact.content_hash != self.world_digest
            || !hash_ok(&self.replay_identity)
            || !hash_ok(&self.world_digest)
        {
            return Err(KnowledgeRepresentationAssuranceError::Artifact(
                "artifact metadata or digest is inconsistent".into(),
            ));
        }
        Ok(())
    }
}

pub fn assure_knowledge_representation(
    request: &ScopedResearchClaims4,
    claims: &[ResearchClaim4],
    peers: &[KnowledgePeer4],
) -> Result<TypedKnowledgeWorld7, KnowledgeRepresentationAssuranceError> {
    if request.schema_version != INPUT_SCHEMA
        || request.request_id.trim().is_empty()
        || request.world_id.trim().is_empty()
        || request.federation_id.trim().is_empty()
        || request.requester.trim().is_empty()
        || request.purpose.trim().is_empty()
        || request.semantic_profile.trim().is_empty()
        || request.checkpoint == 0
        || request.required_claim_order.is_empty()
        || request.required_source_order.is_empty()
        || request.required_peer_order.is_empty()
        || !ordered(&request.required_claim_order)
        || !ordered(&request.required_source_order)
        || !ordered(&request.required_peer_order)
        || !hash_ok(&request.replay_identity)
        || request.budget_units == 0
        || !request.raw_data_local
        || !request.aggregate_only
        || request.boundary != PRECLINICAL_BOUNDARY
        || claims.is_empty()
        || peers.is_empty()
    {
        return Err(KnowledgeRepresentationAssuranceError::Invalid(
            "identity, closure, digest, budget, locality, or boundary is invalid".into(),
        ));
    }
    let mut rows = claims.to_vec();
    rows.sort_by(|a, b| {
        b.confidence_milli
            .cmp(&a.confidence_milli)
            .then(a.claim_id.cmp(&b.claim_id))
    });
    let ids = rows.iter().map(|x| x.claim_id.clone()).collect::<Vec<_>>();
    if ids.windows(2).any(|w| w[0] == w[1]) || ids.iter().any(|x| x.trim().is_empty()) {
        return Err(KnowledgeRepresentationAssuranceError::Invalid(
            "claim identifiers must be unique and non-empty".into(),
        ));
    }
    let mut q = BTreeSet::new();
    let mut u = BTreeSet::new();
    let mut b = BTreeSet::new();
    let mut missing = BTreeSet::new();
    let mut sources = BTreeSet::new();
    let mut selected_sources = BTreeSet::new();
    let mut omission = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    let mut negative = BTreeSet::new();
    let mut provenance = BTreeSet::new();
    let mut scores = Vec::new();
    for c in &rows {
        sources.insert(c.source_id.clone());
        provenance.insert(c.provenance_digest.clone());
        omission.extend(
            c.omission_order
                .iter()
                .map(|x| format!("{}:{x}", c.claim_id)),
        );
        uncertainty.extend(
            c.uncertainty_order
                .iter()
                .map(|x| format!("{}:{x}", c.claim_id)),
        );
        if c.negative_result
            || matches!(
                c.evidence_state,
                KnowledgeEvidenceState::Negative | KnowledgeEvidenceState::Contradicted
            )
        {
            negative.insert(format!("{}:negative-result", c.claim_id));
        }
        let complete = request
            .required_claim_order
            .iter()
            .all(|x| x == &c.claim_id || rows.iter().any(|r| r.claim_id == *x));
        if !complete {
            missing.insert(c.claim_id.clone());
        } else if !c.signed
            || !c.permitted
            || !c.raw_data_local
            || !c.aggregate_only
            || c.semantic_profile != request.semantic_profile
            || c.replay_identity != request.replay_identity
        {
            b.insert(c.claim_id.clone());
        } else if matches!(
            c.evidence_state,
            KnowledgeEvidenceState::Proven | KnowledgeEvidenceState::Supported
        ) && c.confidence_milli >= 600
        {
            q.insert(c.claim_id.clone());
            selected_sources.insert(c.source_id.clone());
            scores.push(c.confidence_milli);
        } else {
            u.insert(c.claim_id.clone());
        }
    }
    let mut peer_rows = peers.to_vec();
    peer_rows.sort_by(|a, b| a.peer_id.cmp(&b.peer_id));
    let peer_ids = peer_rows
        .iter()
        .map(|x| x.peer_id.clone())
        .collect::<Vec<_>>();
    if peer_ids.windows(2).any(|w| w[0] == w[1]) || peer_ids.iter().any(|x| x.trim().is_empty()) {
        return Err(KnowledgeRepresentationAssuranceError::Invalid(
            "peer identifiers must be unique and non-empty".into(),
        ));
    }
    let mut qp = BTreeSet::new();
    let mut mp = BTreeSet::new();
    for p in &peer_rows {
        if p.semantic_profile == request.semantic_profile
            && p.checkpoint == request.checkpoint
            && p.signed
            && p.permitted
            && p.raw_data_local
            && p.aggregate_only
            && p.replay_identity == request.replay_identity
            && matches!(
                p.evidence_state,
                KnowledgeEvidenceState::Proven | KnowledgeEvidenceState::Supported
            )
        {
            qp.insert(p.peer_id.clone());
        } else {
            mp.insert(p.peer_id.clone());
            omission.extend(
                p.omission_order
                    .iter()
                    .map(|x| format!("{}:{x}", p.peer_id)),
            );
            uncertainty.extend(
                p.uncertainty_order
                    .iter()
                    .map(|x| format!("{}:{x}", p.peer_id)),
            );
            if p.negative_result
                || matches!(
                    p.evidence_state,
                    KnowledgeEvidenceState::Negative | KnowledgeEvidenceState::Contradicted
                )
            {
                negative.insert(format!("{}:negative-result", p.peer_id));
            }
        }
    }
    let global = !request.policy_allow
        || !request.protected_closure
        || !request.federation_allow
        || !request.signed_approval
        || !request.raw_data_local
        || !request.aggregate_only
        || !request.adversarial_event_order.is_empty();
    if global {
        b.extend(ids.iter().cloned());
        q.clear();
        u.clear();
        missing.clear();
        scores.clear();
        omission.insert("request:governance-or-adversarial-blocked".into());
    }
    uncertainty.extend(
        request
            .adversarial_event_order
            .iter()
            .map(|e| format!("adversarial:{e}")),
    );
    let required_missing = request
        .required_claim_order
        .iter()
        .filter(|id| !q.contains(*id))
        .map(|id| format!("required:{id}"))
        .collect::<Vec<_>>();
    omission.extend(required_missing);
    if !qp.iter().any(|id| request.required_peer_order.contains(id)) {
        omission.insert("request:peer-quorum-not-met".into());
    }
    let disposition = if global {
        "blocked"
    } else if !u.is_empty()
        || !b.is_empty()
        || !missing.is_empty()
        || qp.len() < request.required_peer_order.len()
    {
        "unresolved"
    } else {
        "qualified"
    };
    if disposition != "qualified" {
        omission.insert("request:knowledge-closure-not-ready".into());
    }
    let payload = json!({"schema_version":"aurora-research-contract/1.0","contract_version":CONTRACT_VERSION,"feature_id":FEATURE_ID,"request_id":request.request_id,"world_id":request.world_id,"federation_id":request.federation_id,"requester":request.requester,"purpose":request.purpose,"semantic_profile":request.semantic_profile,"checkpoint":request.checkpoint,"disposition":disposition,"claim_order":ids,"selected_claim_order":q.iter().cloned().collect::<Vec<_>>() ,"unresolved_claim_order":u.iter().cloned().collect::<Vec<_>>(),"blocked_claim_order":b.iter().cloned().collect::<Vec<_>>(),"missing_claim_order":missing.iter().cloned().collect::<Vec<_>>(),"source_order":sources.iter().cloned().collect::<Vec<_>>(),"selected_source_order":selected_sources.iter().cloned().collect::<Vec<_>>(),"missing_source_order":sources.difference(&selected_sources).cloned().collect::<Vec<_>>(),"peer_order":peer_ids,"qualified_peer_order":qp.iter().cloned().collect::<Vec<_>>(),"missing_peer_order":mp.iter().cloned().collect::<Vec<_>>(),"omission_order":omission.iter().cloned().collect::<Vec<_>>(),"uncertainty_order":uncertainty.iter().cloned().collect::<Vec<_>>(),"negative_evidence_order":negative.iter().cloned().collect::<Vec<_>>(),"confidence_scores_milli":scores,"replay_identity":request.replay_identity,"raw_data_local":true,"aggregate_only":true,"boundary":PRECLINICAL_BOUNDARY});
    let world_digest = digest(&payload)?;
    let artifact = TypedKnowledgeWorld7Artifact {
        artifact_id: format!("typed-knowledge-world-7:{}", request.request_id),
        content_type: CONTENT_TYPE.into(),
        content_hash: world_digest.clone(),
        semantic_loss: payload["omission_order"]
            .as_array()
            .cloned()
            .unwrap_or_default()
            .into_iter()
            .filter_map(|v| v.as_str().map(String::from))
            .collect(),
        provenance_digests: provenance.into_iter().collect(),
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    let receipt = TypedKnowledgeWorld7 {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
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
            .filter_map(|v| v.as_str().map(String::from))
            .collect(),
        selected_claim_order: payload["selected_claim_order"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|v| v.as_str().map(String::from))
            .collect(),
        unresolved_claim_order: payload["unresolved_claim_order"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|v| v.as_str().map(String::from))
            .collect(),
        blocked_claim_order: payload["blocked_claim_order"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|v| v.as_str().map(String::from))
            .collect(),
        missing_claim_order: payload["missing_claim_order"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|v| v.as_str().map(String::from))
            .collect(),
        source_order: payload["source_order"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|v| v.as_str().map(String::from))
            .collect(),
        selected_source_order: payload["selected_source_order"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|v| v.as_str().map(String::from))
            .collect(),
        missing_source_order: payload["missing_source_order"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|v| v.as_str().map(String::from))
            .collect(),
        peer_order: payload["peer_order"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|v| v.as_str().map(String::from))
            .collect(),
        qualified_peer_order: payload["qualified_peer_order"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|v| v.as_str().map(String::from))
            .collect(),
        missing_peer_order: payload["missing_peer_order"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|v| v.as_str().map(String::from))
            .collect(),
        omission_order: payload["omission_order"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|v| v.as_str().map(String::from))
            .collect(),
        uncertainty_order: payload["uncertainty_order"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|v| v.as_str().map(String::from))
            .collect(),
        negative_evidence_order: payload["negative_evidence_order"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|v| v.as_str().map(String::from))
            .collect(),
        confidence_scores_milli: payload["confidence_scores_milli"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|v| v.as_u64().map(|x| x as u16))
            .collect(),
        replay_identity: request.replay_identity.clone(),
        world_digest: world_digest.clone(),
        artifact,
        effect_receipts: vec!["block:unsafe-release".into()],
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
    fn req() -> ScopedResearchClaims4 {
        let x = h("knowledge");
        ScopedResearchClaims4 {
            schema_version: INPUT_SCHEMA.into(),
            request_id: "r".into(),
            world_id: "w".into(),
            federation_id: "f".into(),
            requester: "computational-biologist".into(),
            purpose: "knowledge".into(),
            semantic_profile: "profile:v1".into(),
            checkpoint: 1,
            required_claim_order: vec!["c".into()],
            required_source_order: vec!["s".into()],
            required_peer_order: vec!["p".into()],
            replay_identity: x,
            policy_allow: true,
            protected_closure: true,
            federation_allow: true,
            signed_approval: true,
            aggregate_only: true,
            raw_data_local: true,
            budget_units: 10,
            adversarial_event_order: vec![],
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }
    fn claim() -> ResearchClaim4 {
        let x = h("knowledge");
        ResearchClaim4 {
            claim_id: "c".into(),
            subject: "organoid".into(),
            predicate: "expresses".into(),
            object: "marker".into(),
            source_id: "s".into(),
            study_id: "study".into(),
            modality: "omics".into(),
            semantic_profile: "profile:v1".into(),
            confidence_milli: 800,
            evidence_state: KnowledgeEvidenceState::Supported,
            content_digest: x.clone(),
            provenance_digest: x.clone(),
            replay_identity: x,
            signed: true,
            permitted: true,
            raw_data_local: true,
            aggregate_only: true,
            omission_order: vec![],
            uncertainty_order: vec![],
            negative_result: false,
        }
    }
    fn peer() -> KnowledgePeer4 {
        let x = h("knowledge");
        KnowledgePeer4 {
            peer_id: "p".into(),
            source_order: vec!["s".into()],
            world_digest: x.clone(),
            semantic_profile: "profile:v1".into(),
            checkpoint: 1,
            evidence_state: KnowledgeEvidenceState::Supported,
            signed: true,
            permitted: true,
            raw_data_local: true,
            aggregate_only: true,
            replay_identity: x,
            omission_order: vec![],
            uncertainty_order: vec![],
            negative_result: false,
        }
    }
    #[test]
    fn manifest_a1() {
        assert_eq!(
            knowledge_representation_assurance_manifest().autonomy_tier,
            AutonomyTier::A1
        )
    }
    #[test]
    fn receipt_is_blocked_explicitly() {
        let r = assure_knowledge_representation(&req(), &[claim()], &[peer()]).unwrap();
        assert_eq!(r.effect_receipts, vec!["block:unsafe-release"])
    }
    #[test]
    fn policy_is_fail_closed() {
        let mut r = req();
        r.policy_allow = false;
        assert_eq!(
            assure_knowledge_representation(&r, &[claim()], &[peer()])
                .unwrap()
                .disposition,
            "blocked"
        )
    }
}
