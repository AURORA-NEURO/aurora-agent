//! Federated continual knowledge-representation control for metamorphic mutation families.
//!
//! Atlas feature `AFA-mutation-P04-F32`. The control plane admits mutation-derived knowledge
//! records by digest and oracle evidence only; it never transports source worlds or creates a
//! mutation without the institution-local mutation runtime.

use bioprism_foundation::{
    AuthorityRequirement, AutonomyTier, CapabilityManifest, Determinism, Effect, EvidenceReference,
    EvidenceState, LossSeverity, ProvenanceLink, ResearchSurface, SemanticLoss, TypedPort,
    TypedResearchArtifact, PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION,
};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::{BTreeSet, HashMap};
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-mutation-P04-F32";
pub const FEATURE_VERSION: &str =
    "mutation-federated-continual-knowledge-representation-control-plane/1.0";
pub const INPUT_SCHEMA: &str = "MutationKnowledgeFederatedBatch1@1";
pub const OUTPUT_SCHEMA: &str = "MutationKnowledgeFederatedReceipt1@1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MutationKnowledgeCandidate {
    pub mutation_id: String,
    pub origin: String,
    pub scope: String,
    pub semantic_profile: String,
    pub parent_digest: ContentHash,
    pub instance_digest: ContentHash,
    pub relation_digest: ContentHash,
    pub knowledge_digest: ContentHash,
    pub replay_identity: ContentHash,
    pub evidence_state: EvidenceState,
    pub freshness_seq: u64,
    pub omission_count: u32,
    pub negative_result: bool,
    pub policy_allow: bool,
    pub protected_closure: bool,
    pub oracle_verified: bool,
    pub raw_data_local: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MutationKnowledgeFederatedControlRequest {
    pub request_id: String,
    pub federation_id: String,
    pub purpose: String,
    pub semantic_profile: String,
    pub required_origin_quorum: u32,
    pub capacity: u32,
    pub active_runs: u32,
    pub checkpoint_seq: u64,
    pub candidates: Vec<MutationKnowledgeCandidate>,
    pub policy_allow: bool,
    pub protected_closure: bool,
    pub signed_approval: bool,
    pub approval_token: String,
    pub raw_data_local: bool,
    pub network_permitted: bool,
    pub replay_identity: ContentHash,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MutationKnowledgeDecision {
    pub mutation_id: String,
    pub origin: String,
    pub score: i32,
    pub disposition: String,
    pub failed_gates: Vec<String>,
    pub conditional_gates: Vec<String>,
    pub negative_result: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MutationKnowledgeFederatedReceipt {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub federation_id: String,
    pub purpose: String,
    pub semantic_profile: String,
    pub admission: String,
    pub origin_order: Vec<String>,
    pub admitted_origin_order: Vec<String>,
    pub mutation_order: Vec<String>,
    pub rank_order: Vec<String>,
    pub admitted_order: Vec<String>,
    pub conditional_order: Vec<String>,
    pub blocked_order: Vec<String>,
    pub unknown_order: Vec<String>,
    pub decisions: Vec<MutationKnowledgeDecision>,
    pub checkpoint_seq: u64,
    pub checkpoint_digest: ContentHash,
    pub control_digest: ContentHash,
    pub replay_identity: ContentHash,
    pub semantic_loss: Vec<SemanticLoss>,
    pub omissions: Vec<String>,
    pub uncertainty: Vec<String>,
    pub negative_evidence: Vec<String>,
    pub effect_receipts: Vec<String>,
    pub artifact: TypedResearchArtifact,
    pub raw_data_local: bool,
    pub boundary: String,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum MutationKnowledgeFederatedControlError {
    #[error("invalid mutation knowledge federation request: {0}")]
    Invalid(String),
    #[error("mutation knowledge federation artifact failed: {0}")]
    Artifact(String),
    #[error("mutation knowledge federation serialization failed: {0}")]
    Serialization(String),
}

fn digest(value: &ContentHash) -> bool {
    value.as_str().len() == 64 && value.as_str().bytes().all(|byte| byte.is_ascii_hexdigit())
}
fn canonical(values: &[String]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}
fn sorted_unique(mut values: Vec<String>) -> Vec<String> {
    values.sort();
    values.dedup();
    values
}
fn state_score(state: EvidenceState) -> i32 {
    match state {
        EvidenceState::Proven => 40,
        EvidenceState::Supported => 30,
        EvidenceState::Speculative => 10,
        EvidenceState::Unknown => 0,
        EvidenceState::Contradicted => -40,
    }
}

impl MutationKnowledgeFederatedReceipt {
    pub fn validate(&self) -> Result<(), MutationKnowledgeFederatedControlError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.contract_version != FEATURE_VERSION
            || self.feature_id != FEATURE_ID
            || self.boundary != PRECLINICAL_BOUNDARY
            || !self.raw_data_local
            || self.request_id.trim().is_empty()
            || self.federation_id.trim().is_empty()
            || self.purpose.trim().is_empty()
            || self.semantic_profile.trim().is_empty()
            || self.mutation_order.is_empty()
            || self.decisions.len() != self.mutation_order.len()
            || self.effect_receipts.is_empty()
            || !matches!(
                self.admission.as_str(),
                "admitted" | "approval_required" | "blocked" | "unknown"
            )
            || self.checkpoint_seq == 0
            || !digest(&self.checkpoint_digest)
            || !digest(&self.control_digest)
            || !digest(&self.replay_identity)
        {
            return Err(Self::invalid("mutation federation identity, locality, admission, candidates, checkpoint, effects, or digests are incomplete"));
        }
        for values in [
            &self.origin_order,
            &self.admitted_origin_order,
            &self.mutation_order,
            &self.admitted_order,
            &self.conditional_order,
            &self.blocked_order,
            &self.unknown_order,
            &self.omissions,
            &self.uncertainty,
            &self.negative_evidence,
            &self.effect_receipts,
        ] {
            if !canonical(values) {
                return Err(Self::invalid(
                    "mutation federation ordering is not canonical",
                ));
            }
        }
        if self.rank_order.len() != self.mutation_order.len()
            || self.rank_order.iter().collect::<BTreeSet<_>>().len() != self.mutation_order.len()
            || self
                .rank_order
                .iter()
                .any(|id| !self.mutation_order.contains(id))
        {
            return Err(Self::invalid(
                "mutation rank order is not a candidate permutation",
            ));
        }
        if self
            .decisions
            .iter()
            .map(|decision| decision.mutation_id.as_str())
            .collect::<Vec<_>>()
            != self
                .mutation_order
                .iter()
                .map(String::as_str)
                .collect::<Vec<_>>()
        {
            return Err(Self::invalid(
                "mutation decisions do not match mutation order",
            ));
        }
        let classified = self
            .admitted_order
            .iter()
            .chain(self.conditional_order.iter())
            .chain(self.blocked_order.iter())
            .chain(self.unknown_order.iter())
            .cloned()
            .collect::<BTreeSet<_>>();
        if classified.len() != self.mutation_order.len()
            || classified
                .iter()
                .any(|id| !self.mutation_order.contains(id))
        {
            return Err(Self::invalid(
                "mutation dispositions do not partition candidates",
            ));
        }
        if self
            .admitted_origin_order
            .iter()
            .any(|origin| !self.origin_order.contains(origin))
        {
            return Err(Self::invalid("admitted origin is not in origin order"));
        }
        if self.effect_receipts.iter().any(|effect| {
            !effect.starts_with("operate:mutation-knowledge:")
                && !effect.starts_with("approval-required:")
                && effect != "block:unsafe-release"
        }) {
            return Err(Self::invalid(
                "mutation effect is outside the governed gate",
            ));
        }
        self.artifact
            .validate_metadata()
            .map_err(|error| MutationKnowledgeFederatedControlError::Artifact(error.to_string()))
    }
    fn invalid(message: &str) -> MutationKnowledgeFederatedControlError {
        MutationKnowledgeFederatedControlError::Invalid(message.into())
    }
    pub fn digest(&self) -> Result<ContentHash, MutationKnowledgeFederatedControlError> {
        self.validate()?;
        let value = serde_json::to_value(self).map_err(|error| {
            MutationKnowledgeFederatedControlError::Serialization(error.to_string())
        })?;
        ContentHash::of_value(&value).map_err(|error| {
            MutationKnowledgeFederatedControlError::Serialization(error.to_string())
        })
    }
}

pub fn mutation_knowledge_federated_control_manifest() -> CapabilityManifest {
    CapabilityManifest {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        capability_id: FEATURE_ID.into(),
        version: FEATURE_VERSION.into(),
        owner_crate: "mutation".into(),
        consumers: ["research program lead".into(), "knowledge representation operator".into(), "federation scheduler".into()].into(),
        behavior: "admits mutation-derived knowledge records through deterministic oracle, scope, provenance, policy, closure, approval, locality, quorum, and replay gates".into(),
        value: "turns metamorphic mutation families into portable, honest knowledge representations without exporting source worlds or hiding failed relations".into(),
        inputs: vec![TypedPort { name: "mutation_knowledge_federated_batch".into(), schema: INPUT_SCHEMA.into(), required: true }],
        outputs: vec![TypedPort { name: "mutation_knowledge_federated_receipt".into(), schema: OUTPUT_SCHEMA.into(), required: true }],
        effects: [Effect::ReadLocalData, Effect::ExecuteLocalComputation, Effect::WriteLocalArtifact, Effect::FederationExport].into(),
        permissions: ["operate:mutation-knowledge".into(), "export:mutation-knowledge-digest".into()].into(),
        determinism: Determinism::ByteStable,
        evidence: vec![EvidenceReference { source_id: "w3c-prov-o".into(), state: EvidenceState::Supported, locator: Some("https://www.w3.org/TR/prov-o/".into()) }, EvidenceReference { source_id: "ro-crate-1.3".into(), state: EvidenceState::Supported, locator: Some("https://www.researchobject.org/ro-crate/specification.html".into()) }],
        authority_requirements: vec![AuthorityRequirement { role: "research program lead".into(), reason: "federated mutation-derived knowledge exchange changes shared research state and requires explicit approval".into() }],
        autonomy_tier: AutonomyTier::A2,
        surfaces: [ResearchSurface::Ui, ResearchSurface::Cli, ResearchSurface::Api, ResearchSurface::Sdk, ResearchSurface::McpTool, ResearchSurface::Protocol, ResearchSurface::Policy, ResearchSurface::Operator].into(),
        boundary: PRECLINICAL_BOUNDARY.into(),
    }
}

pub fn operate_mutation_knowledge_federated_control(
    request: &MutationKnowledgeFederatedControlRequest,
) -> Result<MutationKnowledgeFederatedReceipt, MutationKnowledgeFederatedControlError> {
    if request.request_id.trim().is_empty()
        || request.federation_id.trim().is_empty()
        || request.purpose.trim().is_empty()
        || request.semantic_profile.trim().is_empty()
        || request.required_origin_quorum == 0
        || request.capacity == 0
        || request.active_runs > request.capacity
        || request.checkpoint_seq == 0
        || request.candidates.is_empty()
        || !request.raw_data_local
        || request.boundary != PRECLINICAL_BOUNDARY
        || !digest(&request.replay_identity)
    {
        return Err(MutationKnowledgeFederatedControlError::Invalid("request identity, purpose/profile, quorum, capacity, checkpoint, candidates, locality, replay, or boundary is invalid".into()));
    }
    if request.signed_approval && request.approval_token.trim().is_empty() {
        return Err(MutationKnowledgeFederatedControlError::Invalid(
            "signed approval requires an approval token".into(),
        ));
    }
    let mut candidates = request.candidates.clone();
    candidates.sort_by(|left, right| left.mutation_id.cmp(&right.mutation_id));
    if candidates
        .windows(2)
        .any(|pair| pair[0].mutation_id == pair[1].mutation_id)
        || candidates.iter().any(|candidate| {
            candidate.mutation_id.trim().is_empty()
                || candidate.origin.trim().is_empty()
                || candidate.scope.trim().is_empty()
                || candidate.semantic_profile.trim().is_empty()
                || !digest(&candidate.parent_digest)
                || !digest(&candidate.instance_digest)
                || !digest(&candidate.relation_digest)
                || !digest(&candidate.knowledge_digest)
                || !digest(&candidate.replay_identity)
        })
    {
        return Err(MutationKnowledgeFederatedControlError::Invalid(
            "mutation identities and typed digests must be unique, non-empty, and valid".into(),
        ));
    }
    let origin_order = sorted_unique(
        candidates
            .iter()
            .map(|candidate| candidate.origin.clone())
            .collect(),
    );
    let mut global_failed = BTreeSet::new();
    for (gate, failed) in [
        ("policy-allow", !request.policy_allow),
        ("protected-closure", !request.protected_closure),
        ("signed-approval", !request.signed_approval),
        ("network-permission", !request.network_permitted),
        (
            "origin-quorum",
            origin_order.len() < request.required_origin_quorum as usize,
        ),
    ] {
        if failed {
            global_failed.insert(gate.to_string());
        }
    }
    let mut semantic_loss = Vec::new();
    let mut decisions = Vec::with_capacity(candidates.len());
    let mut admitted = Vec::new();
    let mut conditional = Vec::new();
    let mut blocked = Vec::new();
    let unknown = Vec::new();
    let mut omissions = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    let mut negative = BTreeSet::new();
    let mut score_by_id = HashMap::new();
    for candidate in &candidates {
        let mut failed = global_failed.clone();
        let mut pending = BTreeSet::new();
        if candidate.purpose_is_not_supported() {
            failed.insert("candidate-purpose".into());
        }
        if candidate.semantic_profile != request.semantic_profile {
            failed.insert("semantic-profile".into());
        }
        if candidate.replay_identity != request.replay_identity {
            failed.insert("replay-identity".into());
        }
        if !candidate.policy_allow {
            failed.insert("candidate-policy".into());
        }
        if !candidate.protected_closure {
            failed.insert("candidate-protected-closure".into());
        }
        if !candidate.oracle_verified {
            failed.insert("oracle-verification".into());
        }
        if !candidate.raw_data_local {
            failed.insert("candidate-locality".into());
        }
        let score = state_score(candidate.evidence_state)
            + if candidate.oracle_verified { 20 } else { -20 }
            + i32::try_from(candidate.freshness_seq.min(20)).unwrap_or(20)
            - i32::try_from(candidate.omission_count.min(20)).unwrap_or(20) * 2;
        score_by_id.insert(candidate.mutation_id.clone(), score);
        match candidate.evidence_state {
            EvidenceState::Contradicted => {
                failed.insert("contradicted-evidence".into());
            }
            EvidenceState::Unknown | EvidenceState::Speculative => {
                pending.insert("evidence-state".into());
                uncertainty.insert(format!("{}:evidence-state", candidate.mutation_id));
            }
            EvidenceState::Proven | EvidenceState::Supported => {}
        }
        if candidate.omission_count > 0 {
            pending.insert("omission-closure".into());
            omissions.insert(format!(
                "{}:omissions={}",
                candidate.mutation_id, candidate.omission_count
            ));
        }
        negative.insert(format!(
            "{}:{}",
            candidate.mutation_id,
            if candidate.negative_result {
                "negative-result"
            } else {
                "negative-result-not-observed"
            }
        ));
        let disposition = if failed.is_empty() && pending.is_empty() {
            admitted.push(candidate.mutation_id.clone());
            "admitted"
        } else if failed.is_empty() {
            conditional.push(candidate.mutation_id.clone());
            "conditional"
        } else {
            blocked.push(candidate.mutation_id.clone());
            "blocked"
        };
        decisions.push(MutationKnowledgeDecision {
            mutation_id: candidate.mutation_id.clone(),
            origin: candidate.origin.clone(),
            score,
            disposition: disposition.into(),
            failed_gates: failed.into_iter().collect(),
            conditional_gates: pending.into_iter().collect(),
            negative_result: candidate.negative_result,
        });
        if !decisions
            .last()
            .is_some_and(|decision| decision.failed_gates.is_empty())
        {
            semantic_loss.push(SemanticLoss {
                field: format!("mutation:{}", candidate.mutation_id),
                reason: "mutation-derived knowledge failed one or more federation gates".into(),
                severity: LossSeverity::DecisionRelevant,
            });
        }
    }
    let mutation_order = candidates
        .iter()
        .map(|candidate| candidate.mutation_id.clone())
        .collect::<Vec<_>>();
    let mut rank_order = mutation_order.clone();
    rank_order.sort_by(|left, right| {
        score_by_id[right]
            .cmp(&score_by_id[left])
            .then_with(|| left.cmp(right))
    });
    let admitted_origin_order = sorted_unique(
        candidates
            .iter()
            .filter(|candidate| admitted.contains(&candidate.mutation_id))
            .map(|candidate| candidate.origin.clone())
            .collect(),
    );
    let admission = if !global_failed.is_empty() || !blocked.is_empty() {
        "blocked"
    } else if !conditional.is_empty() {
        "approval_required"
    } else if admitted.is_empty() {
        "unknown"
    } else {
        "admitted"
    };
    let checkpoint_digest = ContentHash::of_value(&json!({ "federation_id": request.federation_id, "checkpoint_seq": request.checkpoint_seq, "mutation_order": mutation_order, "origin_order": origin_order })).map_err(|error| MutationKnowledgeFederatedControlError::Serialization(error.to_string()))?;
    let control_digest = ContentHash::of_value(&json!({ "admission": admission, "rank_order": rank_order, "decisions": decisions, "semantic_loss": semantic_loss })).map_err(|error| MutationKnowledgeFederatedControlError::Serialization(error.to_string()))?;
    let effect_receipts = if admission == "admitted" {
        vec![format!(
            "operate:mutation-knowledge:{}",
            request.federation_id
        )]
    } else if admission == "approval_required" {
        vec![
            "approval-required:mutation-knowledge".into(),
            "block:unsafe-release".into(),
        ]
    } else {
        vec!["block:unsafe-release".into()]
    };
    let payload = json!({ "schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "feature_id": FEATURE_ID, "request_id": request.request_id, "federation_id": request.federation_id, "purpose": request.purpose, "semantic_profile": request.semantic_profile, "admission": admission, "mutation_order": mutation_order, "rank_order": rank_order, "decisions": decisions, "checkpoint_digest": checkpoint_digest, "control_digest": control_digest, "replay_identity": request.replay_identity, "semantic_loss": semantic_loss, "omissions": omissions, "uncertainty": uncertainty, "negative_evidence": negative, "boundary": PRECLINICAL_BOUNDARY });
    let artifact = TypedResearchArtifact::from_payload(
        format!("{}:mutation-knowledge", request.request_id),
        "application/vnd.aurora.mutation-knowledge-control+json",
        &payload,
        semantic_loss.clone(),
        vec![ProvenanceLink {
            source_id: request.federation_id.clone(),
            relation: "mutation-knowledge-federated-control".into(),
            digest: control_digest.clone(),
        }],
    )
    .map_err(|error| MutationKnowledgeFederatedControlError::Artifact(error.to_string()))?;
    let receipt = MutationKnowledgeFederatedReceipt {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        contract_version: FEATURE_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        request_id: request.request_id.clone(),
        federation_id: request.federation_id.clone(),
        purpose: request.purpose.clone(),
        semantic_profile: request.semantic_profile.clone(),
        admission: admission.into(),
        origin_order,
        admitted_origin_order,
        mutation_order,
        rank_order,
        admitted_order: sorted_unique(admitted),
        conditional_order: sorted_unique(conditional),
        blocked_order: sorted_unique(blocked),
        unknown_order: unknown,
        decisions,
        checkpoint_seq: request.checkpoint_seq,
        checkpoint_digest,
        control_digest,
        replay_identity: request.replay_identity.clone(),
        semantic_loss,
        omissions: omissions.into_iter().collect(),
        uncertainty: uncertainty.into_iter().collect(),
        negative_evidence: negative.into_iter().collect(),
        effect_receipts: sorted_unique(effect_receipts),
        artifact,
        raw_data_local: true,
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    receipt.validate()?;
    Ok(receipt)
}

impl MutationKnowledgeCandidate {
    fn purpose_is_not_supported(&self) -> bool {
        self.scope.trim().is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn hash(byte: u8) -> ContentHash {
        ContentHash::of_bytes(&[byte; 8])
    }
    fn candidate(id: &str, origin: &str, state: EvidenceState) -> MutationKnowledgeCandidate {
        MutationKnowledgeCandidate {
            mutation_id: id.into(),
            origin: origin.into(),
            scope: "preclinical-culture".into(),
            semantic_profile: "mutation-v1".into(),
            parent_digest: hash(1),
            instance_digest: hash(2),
            relation_digest: hash(3),
            knowledge_digest: hash(4),
            replay_identity: hash(9),
            evidence_state: state,
            freshness_seq: 3,
            omission_count: 0,
            negative_result: true,
            policy_allow: true,
            protected_closure: true,
            oracle_verified: true,
            raw_data_local: true,
        }
    }
    fn request(
        candidates: Vec<MutationKnowledgeCandidate>,
    ) -> MutationKnowledgeFederatedControlRequest {
        MutationKnowledgeFederatedControlRequest {
            request_id: "mutation-control-1".into(),
            federation_id: "fed-mutation".into(),
            purpose: "metamorphic-knowledge".into(),
            semantic_profile: "mutation-v1".into(),
            required_origin_quorum: 2,
            capacity: 8,
            active_runs: 1,
            checkpoint_seq: 3,
            candidates,
            policy_allow: true,
            protected_closure: true,
            signed_approval: true,
            approval_token: "sig".into(),
            raw_data_local: true,
            network_permitted: true,
            replay_identity: hash(9),
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }
    #[test]
    fn manifest_is_a2_and_local_first() {
        let manifest = mutation_knowledge_federated_control_manifest();
        assert_eq!(manifest.autonomy_tier, AutonomyTier::A2);
        assert!(manifest.validate().is_ok());
    }
    #[test]
    fn deterministic_ranking_and_origin_quorum() {
        let receipt = operate_mutation_knowledge_federated_control(&request(vec![
            candidate("m2", "site-b", EvidenceState::Supported),
            candidate("m1", "site-a", EvidenceState::Proven),
        ]))
        .unwrap();
        assert_eq!(receipt.mutation_order, vec!["m1", "m2"]);
        assert_eq!(receipt.rank_order, vec!["m1", "m2"]);
        assert_eq!(receipt.admission, "admitted");
    }
    #[test]
    fn unknown_and_omitted_are_approval_required() {
        let mut unknown = candidate("m1", "site-a", EvidenceState::Unknown);
        unknown.omission_count = 1;
        let receipt = operate_mutation_knowledge_federated_control(&request(vec![
            unknown,
            candidate("m2", "site-b", EvidenceState::Supported),
        ]))
        .unwrap();
        assert_eq!(receipt.admission, "approval_required");
        assert!(receipt.conditional_order.contains(&"m1".into()));
        assert!(receipt
            .effect_receipts
            .contains(&"block:unsafe-release".into()));
    }
    #[test]
    fn contradiction_and_policy_fail_closed() {
        let mut denied = candidate("m1", "site-a", EvidenceState::Contradicted);
        denied.policy_allow = false;
        let receipt = operate_mutation_knowledge_federated_control(&request(vec![
            denied,
            candidate("m2", "site-b", EvidenceState::Supported),
        ]))
        .unwrap();
        assert_eq!(receipt.admission, "blocked");
        assert_eq!(receipt.effect_receipts, vec!["block:unsafe-release"]);
    }
    #[test]
    fn missing_approval_is_blocked_and_raw_data_stays_local() {
        let mut request = request(vec![
            candidate("m1", "site-a", EvidenceState::Supported),
            candidate("m2", "site-b", EvidenceState::Supported),
        ]);
        request.signed_approval = false;
        request.approval_token.clear();
        let receipt = operate_mutation_knowledge_federated_control(&request).unwrap();
        assert_eq!(receipt.admission, "blocked");
        assert!(receipt.raw_data_local);
    }
}
