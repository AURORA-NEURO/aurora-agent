//! Federated continual context-compilation workflow fabric.
//!
//! Atlas feature: `AFA-brain-P03-F16`. Institutions contribute signed,
//! digest-only workflow attestations. A federated Decision Section is scheduled
//! only when a fresh peer quorum covers every required workflow stage.

use bioprism_foundation::{
    AutonomyTier, AuthorityRequirement, CapabilityManifest, Determinism, Effect,
    EvidenceReference, EvidenceState, ResearchSurface, TypedPort, TypedResearchArtifact,
    PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION,
};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-brain-P03-F16";
pub const CONTRACT_VERSION: &str = "brain-federated-context-workflow-fabric/1.0";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederatedContextWorkflowPeer {
    pub institution_id: String,
    pub epoch: u64,
    pub stage_order: Vec<String>,
    pub context_digest: ContentHash,
    pub section_digest: ContentHash,
    pub replay_identity: ContentHash,
    pub evidence_state: EvidenceState,
    pub policy_allow: bool,
    pub protected_closure: bool,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederatedContextWorkflowRequest {
    pub request_id: String,
    pub federation_id: String,
    pub workflow_id: String,
    pub query_id: String,
    pub goal: String,
    pub semantic_profile: String,
    pub required_institution_ids: Vec<String>,
    pub required_stage_ids: Vec<String>,
    pub peers: Vec<FederatedContextWorkflowPeer>,
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
pub struct FederatedContextWorkflowReceipt {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub federation_id: String,
    pub workflow_id: String,
    pub query_id: String,
    pub goal: String,
    pub semantic_profile: String,
    pub disposition: String,
    pub institution_order: Vec<String>,
    pub qualified_institution_order: Vec<String>,
    pub stale_institution_order: Vec<String>,
    pub blocked_institution_order: Vec<String>,
    pub unknown_institution_order: Vec<String>,
    pub required_stage_order: Vec<String>,
    pub scheduled_stage_order: Vec<String>,
    pub aggregate_order: Vec<String>,
    pub quorum: u16,
    pub minimum_quorum: u16,
    pub current_epoch: u64,
    pub budget_units: u32,
    pub consumed_budget_units: u32,
    pub checkpoint_digest: ContentHash,
    pub workflow_digest: ContentHash,
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
pub enum FederatedContextWorkflowError {
    #[error("invalid federated context workflow request: {0}")]
    Invalid(String),
    #[error("federated context workflow artifact failed: {0}")]
    Artifact(String),
}

impl FederatedContextWorkflowReceipt {
    pub fn validate(&self) -> Result<(), FederatedContextWorkflowError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.boundary != PRECLINICAL_BOUNDARY
            || !self.raw_data_local
            || !self.aggregate_only
            || self.request_id.trim().is_empty()
            || self.federation_id.trim().is_empty()
            || self.workflow_id.trim().is_empty()
            || self.query_id.trim().is_empty()
            || self.goal.trim().is_empty()
            || self.semantic_profile.trim().is_empty()
            || self.institution_order.len() < 2
            || self.required_stage_order.is_empty()
            || self.scheduled_stage_order.is_empty()
            || self.aggregate_order.is_empty()
            || self.minimum_quorum == 0
            || self.quorum != self.qualified_institution_order.len() as u16
            || self.quorum > self.institution_order.len() as u16
            || self.budget_units == 0
            || self.consumed_budget_units > self.budget_units
            || self.effect_receipts.is_empty()
        {
            return Err(FederatedContextWorkflowError::Invalid(
                "federated workflow identity, stage closure, quorum, budget, locality, or effects are incomplete".into(),
            ));
        }
        for values in [
            &self.institution_order,
            &self.qualified_institution_order,
            &self.stale_institution_order,
            &self.blocked_institution_order,
            &self.unknown_institution_order,
            &self.required_stage_order,
            &self.scheduled_stage_order,
            &self.aggregate_order,
            &self.omissions,
            &self.uncertainty,
            &self.negative_evidence,
            &self.effect_receipts,
        ] {
            if values.windows(2).any(|pair| pair[0] >= pair[1]) {
                return Err(FederatedContextWorkflowError::Invalid(
                    "federated workflow vectors are not canonical".into(),
                ));
            }
        }
        if !self
            .scheduled_stage_order
            .iter()
            .all(|stage| self.required_stage_order.contains(stage))
        {
            return Err(FederatedContextWorkflowError::Invalid(
                "scheduled stages must be required stages".into(),
            ));
        }
        let institutions = self.institution_order.iter().cloned().collect::<BTreeSet<_>>();
        let mut classified = self.qualified_institution_order.iter().cloned().collect::<BTreeSet<_>>();
        classified.extend(self.stale_institution_order.iter().cloned());
        classified.extend(self.blocked_institution_order.iter().cloned());
        classified.extend(self.unknown_institution_order.iter().cloned());
        if classified != institutions {
            return Err(FederatedContextWorkflowError::Invalid(
                "federated peer states do not partition institutions".into(),
            ));
        }
        if self.aggregate_order.iter().any(|value| value.len() != 64) {
            return Err(FederatedContextWorkflowError::Invalid(
                "federated workflow aggregate entries must be digests".into(),
            ));
        }
        for digest in [
            &self.checkpoint_digest,
            &self.workflow_digest,
            &self.federation_envelope_digest,
            &self.replay_identity,
        ] {
            if digest.as_str().len() != 64 {
                return Err(FederatedContextWorkflowError::Invalid(
                    "federated workflow digest is invalid".into(),
                ));
            }
        }
        if self.effect_receipts.iter().any(|effect| {
            !effect.starts_with("schedule:federated-context-workflow:")
                && effect != "block:unsafe-release"
        }) {
            return Err(FederatedContextWorkflowError::Invalid(
                "federated workflow effect is outside schedule gate".into(),
            ));
        }
        self.artifact
            .validate_metadata()
            .map_err(|error| FederatedContextWorkflowError::Artifact(error.to_string()))
    }

    pub fn digest(&self) -> Result<ContentHash, FederatedContextWorkflowError> {
        self.validate()?;
        let value = serde_json::to_value(self)
            .map_err(|error| FederatedContextWorkflowError::Artifact(error.to_string()))?;
        ContentHash::of_value(&value)
            .map_err(|error| FederatedContextWorkflowError::Artifact(error.to_string()))
    }
}

pub fn federated_context_workflow_fabric_manifest() -> CapabilityManifest {
    CapabilityManifest {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        capability_id: FEATURE_ID.into(),
        version: CONTRACT_VERSION.into(),
        owner_crate: "brain".into(),
        consumers: ["agent developer".into(), "federation steward".into(), "research workflow operator".into()].into(),
        behavior: "orchestrates a quorum-gated continual context workflow across policy-separated institutions using digest-only attestations".into(),
        value: "provides a resumable multi-site Decision-Section workflow without moving raw preclinical observations or hiding missing stages".into(),
        inputs: vec![TypedPort { name: "federated_context_workflow_request".into(), schema: "ResearchWorkflowSpec1@1".into(), required: true }],
        outputs: vec![TypedPort { name: "federated_context_workflow_receipt".into(), schema: "FederatedContextWorkflowReceipt1@1".into(), required: true }],
        effects: [Effect::ReadLocalData, Effect::ExecuteLocalComputation, Effect::FederationExport, Effect::WriteLocalArtifact].into(),
        permissions: ["schedule:federated-context-workflow".into()].into(),
        determinism: Determinism::ByteStable,
        evidence: vec![EvidenceReference { source_id: "w3c-prov-o".into(), state: EvidenceState::Supported, locator: Some("https://www.w3.org/TR/prov-o/".into()) }],
        authority_requirements: vec![AuthorityRequirement { role: "federated workflow approver".into(), reason: "authorize purpose-bound digest-only scheduling after stage closure, freshness, quorum, policy, locality, and replay gates close".into() }],
        autonomy_tier: AutonomyTier::A2,
        surfaces: [ResearchSurface::Ui, ResearchSurface::Api, ResearchSurface::Sdk, ResearchSurface::Cli, ResearchSurface::McpTool, ResearchSurface::Policy, ResearchSurface::Operator].into(),
        boundary: PRECLINICAL_BOUNDARY.into(),
    }
}

pub fn compile_federated_context_workflow(
    request: &FederatedContextWorkflowRequest,
) -> Result<FederatedContextWorkflowReceipt, FederatedContextWorkflowError> {
    if request.request_id.trim().is_empty()
        || request.federation_id.trim().is_empty()
        || request.workflow_id.trim().is_empty()
        || request.query_id.trim().is_empty()
        || request.goal.trim().is_empty()
        || request.semantic_profile.trim().is_empty()
        || request.required_institution_ids.len() < 2
        || request.required_stage_ids.is_empty()
        || request.minimum_quorum == 0
        || request.minimum_quorum as usize > request.required_institution_ids.len()
        || request.budget_units == 0
        || request.replay_identity.as_str().len() != 64
        || request.boundary != PRECLINICAL_BOUNDARY
    {
        return Err(FederatedContextWorkflowError::Invalid(
            "federated workflow identity, stage closure, quorum, budget, replay, or boundary is invalid".into(),
        ));
    }
    let institutions = request.required_institution_ids.iter().cloned().collect::<BTreeSet<_>>();
    let stages = request.required_stage_ids.iter().cloned().collect::<BTreeSet<_>>();
    if institutions.len() != request.required_institution_ids.len()
        || stages.len() != request.required_stage_ids.len()
        || institutions.iter().any(|value| value.trim().is_empty())
        || stages.iter().any(|value| value.trim().is_empty())
    {
        return Err(FederatedContextWorkflowError::Invalid(
            "federated institution and stage identifiers must be unique and non-empty".into(),
        ));
    }
    let mut peer_map = BTreeMap::new();
    for peer in &request.peers {
        if peer_map.insert(peer.institution_id.clone(), peer).is_some() {
            return Err(FederatedContextWorkflowError::Invalid(
                "federated peer attestations must be unique".into(),
            ));
        }
    }
    let mut qualified = BTreeSet::new();
    let mut stale = BTreeSet::new();
    let mut blocked = BTreeSet::new();
    let mut unknown = BTreeSet::new();
    let mut aggregate = BTreeSet::new();
    let mut omissions = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    let mut negative = BTreeSet::new();
    for institution in &institutions {
        let Some(peer) = peer_map.get(institution) else {
            unknown.insert(institution.clone());
            omissions.insert(format!("institution:{}:missing-attestation", institution));
            continue;
        };
        if !request.policy_allow
            || !request.protected_closure
            || !request.signed_approval
            || !request.raw_data_local
            || !request.aggregate_only
            || !peer.policy_allow
            || !peer.protected_closure
            || !peer.raw_data_local
            || !peer.aggregate_only
            || peer.boundary != PRECLINICAL_BOUNDARY
        {
            blocked.insert(institution.clone());
            omissions.insert(format!("institution:{}:federation-gate-blocked", institution));
            continue;
        }
        let peer_stages = peer.stage_order.iter().cloned().collect::<BTreeSet<_>>();
        if !stages.is_subset(&peer_stages) {
            blocked.insert(institution.clone());
            for stage in stages.difference(&peer_stages) {
                omissions.insert(format!("institution:{}:missing-stage:{}", institution, stage));
            }
            continue;
        }
        if peer.replay_identity != request.replay_identity {
            unknown.insert(institution.clone());
            uncertainty.insert(format!("institution:{}:replay-mismatch", institution));
            continue;
        }
        if peer.epoch > request.current_epoch
            || request.current_epoch.saturating_sub(peer.epoch) > request.max_epoch_lag
        {
            stale.insert(institution.clone());
            omissions.insert(format!("institution:{}:stale-epoch", institution));
            continue;
        }
        match peer.evidence_state {
            EvidenceState::Proven | EvidenceState::Supported => {
                qualified.insert(institution.clone());
                let digest = ContentHash::of_value(&json!({
                    "institution_id": institution,
                    "epoch": peer.epoch,
                    "stage_order": stages,
                    "context_digest": peer.context_digest,
                    "section_digest": peer.section_digest,
                    "replay_identity": peer.replay_identity,
                }))
                .map_err(|error| FederatedContextWorkflowError::Artifact(error.to_string()))?;
                aggregate.insert(digest.to_string());
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
    let quorum = qualified.len() as u16;
    let required_budget = (stages.len() as u32).saturating_mul(request.minimum_quorum as u32);
    let gates_open = request.policy_allow
        && request.protected_closure
        && request.signed_approval
        && request.raw_data_local
        && request.aggregate_only;
    let disposition = if !gates_open {
        "blocked"
    } else if quorum >= request.minimum_quorum && request.budget_units >= required_budget {
        "admitted"
    } else {
        "refinement_required"
    };
    if request.budget_units < required_budget {
        omissions.insert("workflow:budget-exhausted".into());
    }
    if !request.policy_allow { omissions.insert("workflow:policy-denied".into()); }
    if !request.protected_closure { omissions.insert("workflow:protected-closure-incomplete".into()); }
    if !request.signed_approval { omissions.insert("workflow:signed-approval-missing".into()); }
    if !request.raw_data_local { omissions.insert("workflow:raw-data-locality-failed".into()); }
    if !request.aggregate_only { omissions.insert("workflow:aggregate-only-required".into()); }
    let consumed = request.budget_units.min(required_budget);
    let checkpoint_digest = ContentHash::of_value(&json!({
        "workflow_id": request.workflow_id,
        "institution_order": institutions,
        "qualified_order": qualified,
        "stage_order": stages,
        "replay_identity": request.replay_identity,
    }))
    .map_err(|error| FederatedContextWorkflowError::Artifact(error.to_string()))?;
    let workflow_digest = ContentHash::of_value(&json!({
        "workflow_id": request.workflow_id,
        "stage_order": stages,
        "quorum": quorum,
        "minimum_quorum": request.minimum_quorum,
        "budget_units": request.budget_units,
        "consumed_budget_units": consumed,
        "checkpoint_digest": checkpoint_digest,
        "replay_identity": request.replay_identity,
    }))
    .map_err(|error| FederatedContextWorkflowError::Artifact(error.to_string()))?;
    let federation_envelope_digest = ContentHash::of_value(&json!({
        "federation_id": request.federation_id,
        "workflow_id": request.workflow_id,
        "aggregate_order": aggregate,
        "workflow_digest": workflow_digest,
        "raw_data_local": true,
        "aggregate_only": true,
    }))
    .map_err(|error| FederatedContextWorkflowError::Artifact(error.to_string()))?;
    let effects = if disposition == "admitted" {
        vec![format!("schedule:federated-context-workflow:{}", request.workflow_id)]
    } else {
        vec!["block:unsafe-release".into()]
    };
    let payload = json!({
        "schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION,
        "contract_version": CONTRACT_VERSION,
        "feature_id": FEATURE_ID,
        "request_id": request.request_id,
        "federation_id": request.federation_id,
        "workflow_id": request.workflow_id,
        "query_id": request.query_id,
        "goal": request.goal,
        "semantic_profile": request.semantic_profile,
        "disposition": disposition,
        "institution_order": institutions,
        "qualified_institution_order": qualified,
        "stale_institution_order": stale,
        "blocked_institution_order": blocked,
        "unknown_institution_order": unknown,
        "required_stage_order": stages,
        "scheduled_stage_order": stages,
        "aggregate_order": aggregate,
        "quorum": quorum,
        "minimum_quorum": request.minimum_quorum,
        "current_epoch": request.current_epoch,
        "budget_units": request.budget_units,
        "consumed_budget_units": consumed,
        "checkpoint_digest": checkpoint_digest,
        "workflow_digest": workflow_digest,
        "federation_envelope_digest": federation_envelope_digest,
        "replay_identity": request.replay_identity,
        "omissions": omissions,
        "uncertainty": uncertainty,
        "negative_evidence": negative,
        "boundary": PRECLINICAL_BOUNDARY,
    });
    let artifact = TypedResearchArtifact::from_payload(
        format!("brain-federated-context-workflow:{}", request.workflow_id),
        "application/vnd.aurora.federated-context-workflow+json",
        &payload,
        Vec::new(),
        Vec::new(),
    )
    .map_err(|error| FederatedContextWorkflowError::Artifact(error.to_string()))?;
    let receipt = FederatedContextWorkflowReceipt {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        request_id: request.request_id.clone(),
        federation_id: request.federation_id.clone(),
        workflow_id: request.workflow_id.clone(),
        query_id: request.query_id.clone(),
        goal: request.goal.clone(),
        semantic_profile: request.semantic_profile.clone(),
        disposition: disposition.into(),
        institution_order: institutions.into_iter().collect(),
        qualified_institution_order: qualified.into_iter().collect(),
        stale_institution_order: stale.into_iter().collect(),
        blocked_institution_order: blocked.into_iter().collect(),
        unknown_institution_order: unknown.into_iter().collect(),
        required_stage_order: stages.clone().into_iter().collect(),
        scheduled_stage_order: stages.into_iter().collect(),
        aggregate_order: aggregate.into_iter().collect(),
        quorum,
        minimum_quorum: request.minimum_quorum,
        current_epoch: request.current_epoch,
        budget_units: request.budget_units,
        consumed_budget_units: consumed,
        checkpoint_digest,
        workflow_digest,
        federation_envelope_digest,
        replay_identity: request.replay_identity.clone(),
        omissions: omissions.into_iter().collect(),
        uncertainty: uncertainty.into_iter().collect(),
        negative_evidence: negative.into_iter().collect(),
        effect_receipts: effects,
        artifact,
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
    fn hash(value: &str) -> ContentHash { ContentHash::of_bytes(value.as_bytes()) }
    fn request() -> FederatedContextWorkflowRequest {
        let replay = hash("replay");
        let stages = vec!["stage:compile".into(), "stage:project".into()];
        FederatedContextWorkflowRequest {
            request_id: "request:federated-context-workflow".into(), federation_id: "federation:one".into(), workflow_id: "workflow:federated".into(), query_id: "query:one".into(), goal: "compile a federated decision context".into(), semantic_profile: "aurora:decision:v1".into(), required_institution_ids: vec!["institution:a".into(), "institution:b".into()], required_stage_ids: stages.clone(), peers: vec![peer("institution:a", &stages, &replay), peer("institution:b", &stages, &replay)], minimum_quorum: 2, current_epoch: 10, max_epoch_lag: 1, budget_units: 4, replay_identity: replay, policy_allow: true, protected_closure: true, signed_approval: true, raw_data_local: true, aggregate_only: true, boundary: PRECLINICAL_BOUNDARY.into()
        }
    }
    fn peer(institution: &str, stages: &[String], replay: &ContentHash) -> FederatedContextWorkflowPeer { FederatedContextWorkflowPeer { institution_id: institution.into(), epoch: 10, stage_order: stages.to_vec(), context_digest: replay.clone(), section_digest: replay.clone(), replay_identity: replay.clone(), evidence_state: EvidenceState::Supported, policy_allow: true, protected_closure: true, raw_data_local: true, aggregate_only: true, boundary: PRECLINICAL_BOUNDARY.into() } }
    #[test] fn manifest_is_a2_and_authorized() { assert_eq!(federated_context_workflow_fabric_manifest().autonomy_tier, AutonomyTier::A2); assert_eq!(federated_context_workflow_fabric_manifest().authority_requirements.len(), 1); }
    #[test] fn fresh_stage_quorum_admits() { let receipt = compile_federated_context_workflow(&request()).unwrap(); assert_eq!(receipt.disposition, "admitted"); assert_eq!(receipt.quorum, 2); }
    #[test] fn missing_stage_blocks_peer() { let mut value = request(); value.peers[1].stage_order.pop(); let receipt = compile_federated_context_workflow(&value).unwrap(); assert_eq!(receipt.disposition, "refinement_required"); assert!(receipt.omissions.iter().any(|item| item.contains("missing-stage"))); }
    #[test] fn stale_peer_is_explicit() { let mut value = request(); value.peers[1].epoch = 1; let receipt = compile_federated_context_workflow(&value).unwrap(); assert!(receipt.stale_institution_order.contains(&"institution:b".into())); }
    #[test] fn policy_denial_blocks_without_schedule() { let mut value = request(); value.policy_allow = false; let receipt = compile_federated_context_workflow(&value).unwrap(); assert_eq!(receipt.disposition, "blocked"); assert_eq!(receipt.effect_receipts, vec!["block:unsafe-release"]); }
    #[test] fn digest_is_stable() { let receipt = compile_federated_context_workflow(&request()).unwrap(); assert_eq!(receipt.digest().unwrap(), receipt.digest().unwrap()); }
}
