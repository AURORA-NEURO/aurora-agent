//! Federated continual context research workbench.
//!
//! Atlas feature: `AFA-brain-P03-F20`. This is a read-only researcher surface for
//! policy-separated institutions. It exchanges digest-only attestations and never
//! treats a missing, stale, contradictory, or unauthorized peer as a successful vote.

use bioprism_foundation::{
    AuthorityRequirement, AutonomyTier, CapabilityManifest, Determinism, Effect,
    EvidenceReference, EvidenceState, ResearchSurface, TypedPort, TypedResearchArtifact,
    PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION,
};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-brain-P03-F20";
pub const CONTRACT_VERSION: &str = "brain-federated-context-research-workbench/1.0";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederatedContextWorkbenchPeer {
    pub institution_id: String,
    pub epoch: u64,
    pub semantic_profile: String,
    pub context_digest: ContentHash,
    pub section_digest: ContentHash,
    pub replay_identity: ContentHash,
    pub evidence_state: EvidenceState,
    pub policy_allow: bool,
    pub protected_closure: bool,
    pub signed_approval: bool,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederatedContextWorkbenchRequest {
    pub session_id: String,
    pub federation_id: String,
    pub query_id: String,
    pub goal: String,
    pub semantic_profile: String,
    pub required_institution_ids: Vec<String>,
    pub peers: Vec<FederatedContextWorkbenchPeer>,
    pub minimum_quorum: u16,
    pub current_epoch: u64,
    pub max_epoch_lag: u64,
    pub budget_units: u32,
    pub replay_identity: ContentHash,
    pub policy_allow: bool,
    pub protected_closure: bool,
    pub signed_approval: bool,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederatedContextWorkbenchReceipt {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub session_id: String,
    pub federation_id: String,
    pub query_id: String,
    pub goal: String,
    pub semantic_profile: String,
    pub disposition: String,
    pub institution_order: Vec<String>,
    pub qualified_institution_order: Vec<String>,
    pub stale_institution_order: Vec<String>,
    pub blocked_institution_order: Vec<String>,
    pub unknown_institution_order: Vec<String>,
    pub view_order: Vec<String>,
    pub action_order: Vec<String>,
    pub blocked_action_order: Vec<String>,
    pub aggregate_order: Vec<String>,
    pub quorum: u16,
    pub minimum_quorum: u16,
    pub current_epoch: u64,
    pub budget_units: u32,
    pub consumed_budget_units: u32,
    pub checkpoint_digest: ContentHash,
    pub federation_envelope_digest: ContentHash,
    pub replay_identity: ContentHash,
    pub omissions: Vec<String>,
    pub uncertainty: Vec<String>,
    pub negative_evidence: Vec<String>,
    pub effect_receipts: Vec<String>,
    pub artifact: TypedResearchArtifact,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub boundary: String,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum FederatedContextWorkbenchError {
    #[error("invalid federated context workbench request: {0}")]
    Invalid(String),
    #[error("federated context workbench artifact failed: {0}")]
    Artifact(String),
}

impl FederatedContextWorkbenchReceipt {
    pub fn validate(&self) -> Result<(), FederatedContextWorkbenchError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.boundary != PRECLINICAL_BOUNDARY
            || !self.raw_data_local
            || !self.aggregate_only
            || self.session_id.trim().is_empty()
            || self.federation_id.trim().is_empty()
            || self.query_id.trim().is_empty()
            || self.goal.trim().is_empty()
            || self.semantic_profile.trim().is_empty()
            || self.institution_order.len() < 2
            || self.view_order.is_empty()
            || self.action_order.is_empty()
            || self.minimum_quorum == 0
            || self.quorum != self.qualified_institution_order.len() as u16
            || self.quorum > self.institution_order.len() as u16
            || self.budget_units == 0
            || self.consumed_budget_units > self.budget_units
            || self.effect_receipts.is_empty()
            || !matches!(self.disposition.as_str(), "ready" | "needs_refinement" | "blocked")
        {
            return Err(FederatedContextWorkbenchError::Invalid(
                "federated workbench identity, quorum, budget, locality, view, action, or disposition is incomplete".into(),
            ));
        }
        for values in [
            &self.institution_order,
            &self.qualified_institution_order,
            &self.stale_institution_order,
            &self.blocked_institution_order,
            &self.unknown_institution_order,
            &self.view_order,
            &self.action_order,
            &self.blocked_action_order,
            &self.aggregate_order,
            &self.omissions,
            &self.uncertainty,
            &self.negative_evidence,
            &self.effect_receipts,
        ] {
            if values.windows(2).any(|pair| pair[0] >= pair[1]) {
                return Err(FederatedContextWorkbenchError::Invalid(
                    "federated workbench vectors are not canonical".into(),
                ));
            }
        }
        let institutions = self.institution_order.iter().cloned().collect::<BTreeSet<_>>();
        let mut classified = self.qualified_institution_order.iter().cloned().collect::<BTreeSet<_>>();
        classified.extend(self.stale_institution_order.iter().cloned());
        classified.extend(self.blocked_institution_order.iter().cloned());
        classified.extend(self.unknown_institution_order.iter().cloned());
        if classified != institutions {
            return Err(FederatedContextWorkbenchError::Invalid(
                "federated peer states do not partition institutions".into(),
            ));
        }
        if self.aggregate_order.iter().any(|value| value.len() != 64) {
            return Err(FederatedContextWorkbenchError::Invalid(
                "federated workbench aggregate entries must be digests".into(),
            ));
        }
        for digest in [&self.checkpoint_digest, &self.federation_envelope_digest, &self.replay_identity] {
            if digest.as_str().len() != 64 {
                return Err(FederatedContextWorkbenchError::Invalid(
                    "federated workbench digest is invalid".into(),
                ));
            }
        }
        if self.effect_receipts.iter().any(|effect| {
            !effect.starts_with("view:local-federated-context-workbench:") && effect != "block:unsafe-release"
        }) {
            return Err(FederatedContextWorkbenchError::Invalid(
                "federated workbench effect is outside read-only view gate".into(),
            ));
        }
        self.artifact
            .validate_metadata()
            .map_err(|error| FederatedContextWorkbenchError::Artifact(error.to_string()))
    }

    pub fn digest(&self) -> Result<ContentHash, FederatedContextWorkbenchError> {
        self.validate()?;
        let value = serde_json::to_value(self)
            .map_err(|error| FederatedContextWorkbenchError::Artifact(error.to_string()))?;
        ContentHash::of_value(&value)
            .map_err(|error| FederatedContextWorkbenchError::Artifact(error.to_string()))
    }
}

pub fn federated_context_workbench_manifest() -> CapabilityManifest {
    CapabilityManifest {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        capability_id: FEATURE_ID.into(),
        version: CONTRACT_VERSION.into(),
        owner_crate: "brain".into(),
        consumers: ["platform reliability engineer".into(), "research workflow operator".into()].into(),
        behavior: "presents a quorum-gated federated context workbench from signed digest-only peer attestations".into(),
        value: "gives reliability engineers an auditable multi-institution Decision-Section view without moving raw research data or hiding peer failures".into(),
        inputs: vec![TypedPort { name: "federated_context_workbench_request".into(), schema: "ResearchWorkbenchSession1@1".into(), required: true }],
        outputs: vec![TypedPort { name: "federated_context_workbench_receipt".into(), schema: "FederatedContextWorkbenchReceipt1@1".into(), required: true }],
        effects: [Effect::ReadLocalData, Effect::ExecuteLocalComputation, Effect::WriteLocalArtifact].into(),
        permissions: ["view:local-federated-context-workbench".into()].into(),
        determinism: Determinism::ByteStable,
        evidence: vec![EvidenceReference { source_id: "w3c-prov-o".into(), state: EvidenceState::Supported, locator: Some("https://www.w3.org/TR/prov-o/".into()) }],
        authority_requirements: vec![AuthorityRequirement { role: "federated context approver".into(), reason: "authorize purpose-bound digest-only peer context review after quorum, freshness, policy, locality, approval, and replay gates close".into() }],
        autonomy_tier: AutonomyTier::A2,
        surfaces: [ResearchSurface::Ui, ResearchSurface::Api, ResearchSurface::Sdk, ResearchSurface::Cli, ResearchSurface::McpTool, ResearchSurface::Policy, ResearchSurface::Operator].into(),
        boundary: PRECLINICAL_BOUNDARY.into(),
    }
}

pub fn render_federated_context_workbench(
    request: &FederatedContextWorkbenchRequest,
) -> Result<FederatedContextWorkbenchReceipt, FederatedContextWorkbenchError> {
    if request.session_id.trim().is_empty()
        || request.federation_id.trim().is_empty()
        || request.query_id.trim().is_empty()
        || request.goal.trim().is_empty()
        || request.semantic_profile.trim().is_empty()
        || request.required_institution_ids.len() < 2
        || request.minimum_quorum == 0
        || request.minimum_quorum as usize > request.required_institution_ids.len()
        || request.budget_units == 0
        || request.replay_identity.as_str().len() != 64
        || request.boundary != PRECLINICAL_BOUNDARY
    {
        return Err(FederatedContextWorkbenchError::Invalid(
            "federated workbench identity, institutions, quorum, budget, replay, or boundary is invalid".into(),
        ));
    }
    let institutions = request.required_institution_ids.iter().cloned().collect::<BTreeSet<_>>();
    if institutions.len() != request.required_institution_ids.len() || institutions.iter().any(|value| value.trim().is_empty()) {
        return Err(FederatedContextWorkbenchError::Invalid(
            "federated institution identifiers must be unique and non-empty".into(),
        ));
    }
    let mut peers = std::collections::BTreeMap::new();
    for peer in &request.peers {
        if peers.insert(peer.institution_id.clone(), peer).is_some() {
            return Err(FederatedContextWorkbenchError::Invalid("federated peer attestations must be unique".into()));
        }
    }
    let mut qualified = BTreeSet::new();
    let mut stale = BTreeSet::new();
    let mut blocked = BTreeSet::new();
    let mut unknown = BTreeSet::new();
    let mut aggregate = BTreeSet::new();
    let mut views = BTreeSet::from(["view:peer-quorum".to_string(), "view:replay-identity".to_string(), "view:provenance-and-omissions".to_string()]);
    let mut actions = BTreeSet::from(["action:inspect-peer-attestation".to_string(), "action:replay-local-federated-view".to_string()]);
    let mut blocked_actions = BTreeSet::new();
    let mut omissions = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    let mut negative = BTreeSet::new();
    for institution in &institutions {
        let Some(peer) = peers.get(institution) else {
            unknown.insert(institution.clone());
            omissions.insert(format!("institution:{}:missing-attestation", institution));
            continue;
        };
        if !request.policy_allow || !request.protected_closure || !request.signed_approval || !request.raw_data_local || !request.aggregate_only || !peer.policy_allow || !peer.protected_closure || !peer.signed_approval || !peer.raw_data_local || !peer.aggregate_only || peer.boundary != PRECLINICAL_BOUNDARY {
            blocked.insert(institution.clone());
            omissions.insert(format!("institution:{}:federation-gate-blocked", institution));
        } else if peer.semantic_profile != request.semantic_profile {
            blocked.insert(institution.clone());
            negative.insert(format!("institution:{}:semantic-profile-mismatch", institution));
        } else if peer.replay_identity != request.replay_identity {
            unknown.insert(institution.clone());
            uncertainty.insert(format!("institution:{}:replay-mismatch", institution));
        } else if peer.epoch > request.current_epoch || request.current_epoch.saturating_sub(peer.epoch) > request.max_epoch_lag {
            stale.insert(institution.clone());
            omissions.insert(format!("institution:{}:stale-epoch", institution));
        } else {
            match peer.evidence_state {
                EvidenceState::Proven | EvidenceState::Supported => {
                    qualified.insert(institution.clone());
                    aggregate.insert(ContentHash::of_value(&json!({"institution_id": institution, "epoch": peer.epoch, "semantic_profile": request.semantic_profile, "context_digest": peer.context_digest, "section_digest": peer.section_digest, "replay_identity": peer.replay_identity})).map_err(|error| FederatedContextWorkbenchError::Artifact(error.to_string()))?.to_string());
                }
                EvidenceState::Speculative | EvidenceState::Unknown => {
                    unknown.insert(institution.clone());
                    uncertainty.insert(format!("institution:{}:evidence-uncertain", institution));
                }
                EvidenceState::Contradicted => {
                    blocked.insert(institution.clone());
                    negative.insert(format!("institution:{}:contradicted", institution));
                }
            }
        }
    }
    let quorum = qualified.len() as u16;
    let required_budget = request.minimum_quorum as u32;
    let gates_open = request.policy_allow && request.protected_closure && request.signed_approval && request.raw_data_local && request.aggregate_only;
    let disposition = if !gates_open { "blocked" } else if quorum >= request.minimum_quorum && request.budget_units >= required_budget { "ready" } else { "needs_refinement" };
    let consumed = required_budget.min(request.budget_units);
    if request.budget_units < required_budget { omissions.insert("workbench:budget-exhausted".into()); }
    if !request.policy_allow { omissions.insert("workbench:policy-denied".into()); }
    if !request.protected_closure { omissions.insert("workbench:protected-closure-incomplete".into()); }
    if !request.signed_approval { omissions.insert("workbench:signed-approval-missing".into()); }
    if !request.raw_data_local { omissions.insert("workbench:raw-data-locality-failed".into()); }
    if !request.aggregate_only { omissions.insert("workbench:aggregate-only-required".into()); }
    if disposition == "ready" {
        actions.extend(["action:open-decision-section".to_string(), "action:export-digest-only-context".to_string()]);
    } else if disposition == "blocked" {
        blocked_actions.extend(["action:open-decision-section".to_string(), "action:export-digest-only-context".to_string(), "action:replay-local-federated-view".to_string()]);
        actions.clear();
        actions.insert("action:inspect-block-reason".into());
    } else {
        actions.extend(["action:review-peer-outcomes".to_string(), "action:request-federation-refinement".to_string()]);
        uncertainty.insert("workbench:quorum-not-admitted".into());
    }
    if !stale.is_empty() { views.insert("view:stale-peers".into()); }
    if !unknown.is_empty() { views.insert("view:uncertain-peers".into()); }
    if !blocked.is_empty() { views.insert("view:blocked-peers".into()); }
    let checkpoint_digest = ContentHash::of_value(&json!({"session_id": request.session_id, "institution_order": institutions, "qualified_order": qualified, "replay_identity": request.replay_identity})).map_err(|error| FederatedContextWorkbenchError::Artifact(error.to_string()))?;
    let federation_envelope_digest = ContentHash::of_value(&json!({"federation_id": request.federation_id, "aggregate_order": aggregate, "checkpoint_digest": checkpoint_digest, "raw_data_local": true, "aggregate_only": true})).map_err(|error| FederatedContextWorkbenchError::Artifact(error.to_string()))?;
    let artifact_payload = json!({"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "contract_version": CONTRACT_VERSION, "feature_id": FEATURE_ID, "session_id": request.session_id, "federation_id": request.federation_id, "disposition": disposition, "aggregate_order": aggregate, "checkpoint_digest": checkpoint_digest, "federation_envelope_digest": federation_envelope_digest, "replay_identity": request.replay_identity, "boundary": PRECLINICAL_BOUNDARY});
    let artifact = TypedResearchArtifact::from_payload(format!("brain-federated-context-workbench:{}", request.session_id), "application/vnd.aurora.federated-context-workbench+json", &artifact_payload, Vec::new(), Vec::new()).map_err(|error| FederatedContextWorkbenchError::Artifact(error.to_string()))?;
    let receipt = FederatedContextWorkbenchReceipt { schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(), contract_version: CONTRACT_VERSION.into(), feature_id: FEATURE_ID.into(), session_id: request.session_id.clone(), federation_id: request.federation_id.clone(), query_id: request.query_id.clone(), goal: request.goal.clone(), semantic_profile: request.semantic_profile.clone(), disposition: disposition.into(), institution_order: institutions.into_iter().collect(), qualified_institution_order: qualified.into_iter().collect(), stale_institution_order: stale.into_iter().collect(), blocked_institution_order: blocked.into_iter().collect(), unknown_institution_order: unknown.into_iter().collect(), view_order: views.into_iter().collect(), action_order: actions.into_iter().collect(), blocked_action_order: blocked_actions.into_iter().collect(), aggregate_order: aggregate.into_iter().collect(), quorum, minimum_quorum: request.minimum_quorum, current_epoch: request.current_epoch, budget_units: request.budget_units, consumed_budget_units: consumed, checkpoint_digest, federation_envelope_digest, replay_identity: request.replay_identity.clone(), omissions: omissions.into_iter().collect(), uncertainty: uncertainty.into_iter().collect(), negative_evidence: negative.into_iter().collect(), effect_receipts: if disposition == "blocked" { vec!["block:unsafe-release".into()] } else { vec![format!("view:local-federated-context-workbench:{}", request.session_id)] }, artifact, raw_data_local: true, aggregate_only: true, boundary: PRECLINICAL_BOUNDARY.into() };
    receipt.validate()?;
    Ok(receipt)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn hash(value: &str) -> ContentHash { ContentHash::of_bytes(value.as_bytes()) }
    fn request() -> FederatedContextWorkbenchRequest {
        let replay = hash("federated-workbench-replay");
        let peer = |id: &str| FederatedContextWorkbenchPeer { institution_id: id.into(), epoch: 10, semantic_profile: "profile:v1".into(), context_digest: replay.clone(), section_digest: replay.clone(), replay_identity: replay.clone(), evidence_state: EvidenceState::Supported, policy_allow: true, protected_closure: true, signed_approval: true, raw_data_local: true, aggregate_only: true, boundary: PRECLINICAL_BOUNDARY.into() };
        FederatedContextWorkbenchRequest { session_id: "session:federated-workbench".into(), federation_id: "federation:preclinical".into(), query_id: "query:context".into(), goal: "review federated context".into(), semantic_profile: "profile:v1".into(), required_institution_ids: vec!["institution:a".into(), "institution:b".into()], peers: vec![peer("institution:a"), peer("institution:b")], minimum_quorum: 2, current_epoch: 10, max_epoch_lag: 1, budget_units: 2, replay_identity: replay, policy_allow: true, protected_closure: true, signed_approval: true, raw_data_local: true, aggregate_only: true, boundary: PRECLINICAL_BOUNDARY.into() }
    }
    #[test] fn manifest_is_a2_and_authorized() { assert_eq!(federated_context_workbench_manifest().autonomy_tier, AutonomyTier::A2); assert_eq!(federated_context_workbench_manifest().authority_requirements.len(), 1); }
    #[test] fn quorum_is_ready() { let receipt = render_federated_context_workbench(&request()).unwrap(); assert_eq!(receipt.disposition, "ready"); assert_eq!(receipt.quorum, 2); assert_eq!(receipt.aggregate_order.len(), 2); }
    #[test] fn stale_peer_is_explicit() { let mut value = request(); value.peers[1].epoch = 1; let receipt = render_federated_context_workbench(&value).unwrap(); assert!(receipt.stale_institution_order.contains(&"institution:b".into())); assert_eq!(receipt.disposition, "needs_refinement"); }
    #[test] fn semantic_mismatch_is_negative() { let mut value = request(); value.peers[0].semantic_profile = "profile:other".into(); let receipt = render_federated_context_workbench(&value).unwrap(); assert!(receipt.negative_evidence.iter().any(|item| item.contains("semantic-profile-mismatch"))); }
    #[test] fn policy_denial_blocks_actions() { let mut value = request(); value.policy_allow = false; let receipt = render_federated_context_workbench(&value).unwrap(); assert_eq!(receipt.disposition, "blocked"); assert_eq!(receipt.effect_receipts, vec!["block:unsafe-release"]); }
    #[test] fn digest_is_stable() { let receipt = render_federated_context_workbench(&request()).unwrap(); assert_eq!(receipt.digest().unwrap(), receipt.digest().unwrap()); }
}
