//! Federated continual context research workbench.
//!
//! Atlas feature: `AFA-brain-P03-F20`. This is a read-only researcher surface for
//! policy-separated institutions. It exchanges digest-only attestations and never
//! treats a missing, stale, contradictory, or unauthorized peer as a successful vote.

use bioprism_foundation::{
    AuthorityRequirement, AutonomyTier, CapabilityManifest, Determinism, Effect, EvidenceReference,
    EvidenceState, ResearchSurface, TypedPort, TypedResearchArtifact, PRECLINICAL_BOUNDARY,
    RESEARCH_CONTRACT_SCHEMA_VERSION,
};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-brain-P03-F20";
pub const CONTRACT_VERSION: &str = "brain-federated-context-research-workbench/1.0";
const WORKBENCH_CONTENT_TYPE: &str = "application/vnd.aurora.federated-context-workbench+json";
const MAX_TEXT_BYTES: usize = 512;

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
            || self.session_id.trim().is_empty()
            || self.federation_id.trim().is_empty()
            || self.query_id.trim().is_empty()
            || self.goal.trim().is_empty()
            || self.semantic_profile.trim().is_empty()
            || self.institution_order.len() < 2
            || self.view_order.is_empty()
            || self.action_order.is_empty()
            || self.minimum_quorum == 0
            || self.institution_order.len() > usize::from(u16::MAX)
            || usize::from(self.quorum) != self.qualified_institution_order.len()
            || usize::from(self.quorum) > self.institution_order.len()
            || self.budget_units == 0
            || self.consumed_budget_units > self.budget_units
            || self.effect_receipts.is_empty()
            || !matches!(
                self.disposition.as_str(),
                "ready" | "needs_refinement" | "blocked"
            )
        {
            return Err(FederatedContextWorkbenchError::Invalid(
                "federated workbench identity, quorum, budget, locality, view, action, or disposition is incomplete".into(),
            ));
        }
        for (value, field) in [
            (&self.session_id, "session_id"),
            (&self.federation_id, "federation_id"),
            (&self.query_id, "query_id"),
            (&self.goal, "goal"),
            (&self.semantic_profile, "semantic_profile"),
            (&self.disposition, "disposition"),
            (&self.boundary, "boundary"),
        ] {
            validate_text(value, field)?;
        }
        for (values, field) in [
            (&self.institution_order, "institution_order"),
            (
                &self.qualified_institution_order,
                "qualified_institution_order",
            ),
            (&self.stale_institution_order, "stale_institution_order"),
            (&self.blocked_institution_order, "blocked_institution_order"),
            (&self.unknown_institution_order, "unknown_institution_order"),
            (&self.view_order, "view_order"),
            (&self.action_order, "action_order"),
            (&self.blocked_action_order, "blocked_action_order"),
            (&self.aggregate_order, "aggregate_order"),
            (&self.omissions, "omissions"),
            (&self.uncertainty, "uncertainty"),
            (&self.negative_evidence, "negative_evidence"),
            (&self.effect_receipts, "effect_receipts"),
        ] {
            validate_sorted_unique(values, field)?;
        }
        let institutions = self
            .institution_order
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>();
        let mut classified = self
            .qualified_institution_order
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>();
        classified.extend(self.stale_institution_order.iter().cloned());
        classified.extend(self.blocked_institution_order.iter().cloned());
        classified.extend(self.unknown_institution_order.iter().cloned());
        if classified != institutions
            || !identity_keys(&self.qualified_institution_order)
                .is_disjoint(&identity_keys(&self.stale_institution_order))
            || !identity_keys(&self.qualified_institution_order)
                .is_disjoint(&identity_keys(&self.blocked_institution_order))
            || !identity_keys(&self.qualified_institution_order)
                .is_disjoint(&identity_keys(&self.unknown_institution_order))
            || !identity_keys(&self.stale_institution_order)
                .is_disjoint(&identity_keys(&self.blocked_institution_order))
            || !identity_keys(&self.stale_institution_order)
                .is_disjoint(&identity_keys(&self.unknown_institution_order))
            || !identity_keys(&self.blocked_institution_order)
                .is_disjoint(&identity_keys(&self.unknown_institution_order))
        {
            return Err(FederatedContextWorkbenchError::Invalid(
                "federated peer states do not partition institutions".into(),
            ));
        }
        if self.aggregate_order.iter().any(|value| value.len() != 64) {
            return Err(FederatedContextWorkbenchError::Invalid(
                "federated workbench aggregate entries must be digests".into(),
            ));
        }
        for digest in [
            &self.checkpoint_digest,
            &self.federation_envelope_digest,
            &self.replay_identity,
        ] {
            if digest.as_str().len() != 64 {
                return Err(FederatedContextWorkbenchError::Invalid(
                    "federated workbench digest is invalid".into(),
                ));
            }
        }
        if self.effect_receipts.iter().any(|effect| {
            !effect.starts_with("view:local-federated-context-workbench:")
                && effect != "block:unsafe-release"
        }) {
            return Err(FederatedContextWorkbenchError::Invalid(
                "federated workbench effect is outside read-only view gate".into(),
            ));
        }
        let expected_effect_receipts = if self.disposition == "blocked" {
            vec!["block:unsafe-release".into()]
        } else {
            vec![format!(
                "view:local-federated-context-workbench:{}",
                self.session_id
            )]
        };
        if self.effect_receipts != expected_effect_receipts {
            return Err(FederatedContextWorkbenchError::Invalid(
                "federated workbench effect does not match disposition".into(),
            ));
        }
        if !self.raw_data_local {
            return Err(FederatedContextWorkbenchError::Invalid(
                "federated context workbench receipts must declare local emitted data".into(),
            ));
        }
        if !self.aggregate_only
            && (self.disposition != "blocked"
                || !self
                    .omissions
                    .iter()
                    .any(|item| item == "workbench:aggregate-only-required"))
        {
            return Err(FederatedContextWorkbenchError::Invalid(
                "non-aggregate federated workbench must be blocked and retain release evidence"
                    .into(),
            ));
        }
        let expected_checkpoint_digest = ContentHash::of_value(&json!({
            "session_id": self.session_id,
            "institution_order": self.institution_order,
            "qualified_institution_order": self.qualified_institution_order,
            "stale_institution_order": self.stale_institution_order,
            "blocked_institution_order": self.blocked_institution_order,
            "unknown_institution_order": self.unknown_institution_order,
            "view_order": self.view_order,
            "action_order": self.action_order,
            "blocked_action_order": self.blocked_action_order,
            "quorum": self.quorum,
            "minimum_quorum": self.minimum_quorum,
            "current_epoch": self.current_epoch,
            "budget_units": self.budget_units,
            "consumed_budget_units": self.consumed_budget_units,
            "disposition": self.disposition,
            "replay_identity": self.replay_identity,
            "raw_data_local": self.raw_data_local,
            "aggregate_only": self.aggregate_only,
        }))
        .map_err(|error| FederatedContextWorkbenchError::Artifact(error.to_string()))?;
        if self.checkpoint_digest != expected_checkpoint_digest {
            return Err(FederatedContextWorkbenchError::Invalid(
                "federated workbench checkpoint is not bound to peer outcomes".into(),
            ));
        }
        let expected_federation_envelope_digest = ContentHash::of_value(&json!({
            "federation_id": self.federation_id,
            "query_id": self.query_id,
            "goal": self.goal,
            "semantic_profile": self.semantic_profile,
            "aggregate_order": self.aggregate_order,
            "checkpoint_digest": self.checkpoint_digest,
            "replay_identity": self.replay_identity,
            "raw_data_local": self.raw_data_local,
            "aggregate_only": self.aggregate_only,
            "boundary": self.boundary,
        }))
        .map_err(|error| FederatedContextWorkbenchError::Artifact(error.to_string()))?;
        if self.federation_envelope_digest != expected_federation_envelope_digest {
            return Err(FederatedContextWorkbenchError::Invalid(
                "federated workbench envelope is not bound to release metadata".into(),
            ));
        }
        let expected_artifact_id = format!("brain-federated-context-workbench:{}", self.session_id);
        if self.artifact.artifact_id != expected_artifact_id
            || self.artifact.content_type != WORKBENCH_CONTENT_TYPE
            || !self.artifact.semantic_loss.is_empty()
            || !self.artifact.provenance.is_empty()
        {
            return Err(FederatedContextWorkbenchError::Invalid(
                "federated workbench artifact identity or provenance is inconsistent".into(),
            ));
        }
        self.artifact
            .validate_metadata()
            .map_err(|error| FederatedContextWorkbenchError::Artifact(error.to_string()))?;
        self.artifact
            .verify_payload(&receipt_payload(self))
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
        || request.required_institution_ids.len() > usize::from(u16::MAX)
        || request.minimum_quorum == 0
        || usize::from(request.minimum_quorum) > request.required_institution_ids.len()
        || request.budget_units == 0
        || request.replay_identity.as_str().len() != 64
        || request.boundary != PRECLINICAL_BOUNDARY
    {
        return Err(FederatedContextWorkbenchError::Invalid(
            "federated workbench identity, institutions, quorum, budget, replay, or boundary is invalid".into(),
        ));
    }
    for (value, field) in [
        (&request.session_id, "session_id"),
        (&request.federation_id, "federation_id"),
        (&request.query_id, "query_id"),
        (&request.goal, "goal"),
        (&request.semantic_profile, "semantic_profile"),
        (&request.boundary, "boundary"),
    ] {
        validate_text(value, field)?;
    }
    validate_unique(
        &request.required_institution_ids,
        "required_institution_ids",
    )?;
    let institutions = request
        .required_institution_ids
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    if institutions.len() != request.required_institution_ids.len() {
        return Err(FederatedContextWorkbenchError::Invalid(
            "federated institution identifiers must be unique and non-empty".into(),
        ));
    }
    let mut peers = std::collections::BTreeMap::new();
    let mut peer_keys = BTreeSet::new();
    for peer in &request.peers {
        for (value, field) in [
            (&peer.institution_id, "peer.institution_id"),
            (&peer.semantic_profile, "peer.semantic_profile"),
            (&peer.boundary, "peer.boundary"),
        ] {
            validate_text(value, field)?;
        }
        for (digest, field) in [
            (&peer.context_digest, "peer.context_digest"),
            (&peer.section_digest, "peer.section_digest"),
            (&peer.replay_identity, "peer.replay_identity"),
        ] {
            if digest.as_str().len() != 64 {
                return Err(FederatedContextWorkbenchError::Invalid(format!(
                    "{field} must be a 64-character content hash"
                )));
            }
        }
        if !peer_keys.insert(peer.institution_id.to_ascii_lowercase()) {
            return Err(FederatedContextWorkbenchError::Invalid(
                "federated peer attestations must be unique and case-distinct".into(),
            ));
        }
        if peers.insert(peer.institution_id.clone(), peer).is_some() {
            return Err(FederatedContextWorkbenchError::Invalid(
                "federated peer attestations must be unique".into(),
            ));
        }
    }
    let mut qualified = BTreeSet::new();
    let mut stale = BTreeSet::new();
    let mut blocked = BTreeSet::new();
    let mut unknown = BTreeSet::new();
    let mut aggregate = BTreeSet::new();
    let mut views = BTreeSet::from([
        "view:peer-quorum".to_string(),
        "view:replay-identity".to_string(),
        "view:provenance-and-omissions".to_string(),
    ]);
    let mut actions = BTreeSet::from([
        "action:inspect-peer-attestation".to_string(),
        "action:replay-local-federated-view".to_string(),
    ]);
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
        if !request.policy_allow
            || !request.protected_closure
            || !request.signed_approval
            || !request.raw_data_local
            || !request.aggregate_only
            || !peer.policy_allow
            || !peer.protected_closure
            || !peer.signed_approval
            || !peer.raw_data_local
            || !peer.aggregate_only
            || peer.boundary != PRECLINICAL_BOUNDARY
        {
            blocked.insert(institution.clone());
            omissions.insert(format!(
                "institution:{}:federation-gate-blocked",
                institution
            ));
        } else if peer.semantic_profile != request.semantic_profile {
            blocked.insert(institution.clone());
            negative.insert(format!(
                "institution:{}:semantic-profile-mismatch",
                institution
            ));
        } else if peer.replay_identity != request.replay_identity {
            unknown.insert(institution.clone());
            uncertainty.insert(format!("institution:{}:replay-mismatch", institution));
        } else if peer.epoch > request.current_epoch
            || request.current_epoch.saturating_sub(peer.epoch) > request.max_epoch_lag
        {
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
    let quorum = u16::try_from(qualified.len()).map_err(|_| {
        FederatedContextWorkbenchError::Invalid(
            "federated qualified institution count exceeds the receipt quorum width".into(),
        )
    })?;
    let required_budget = u32::from(request.minimum_quorum);
    let consumed = required_budget.min(request.budget_units);
    let locality_failure = !request.raw_data_local
        || institutions
            .iter()
            .filter_map(|institution| peers.get(institution))
            .any(|peer| !peer.raw_data_local);
    let aggregate_only_failure = !request.aggregate_only
        || institutions
            .iter()
            .filter_map(|institution| peers.get(institution))
            .any(|peer| !peer.aggregate_only);
    if locality_failure {
        omissions.insert("workbench:raw-data-locality-failed".into());
    }
    if aggregate_only_failure {
        omissions.insert("workbench:aggregate-only-required".into());
    }
    let locality_gate = !locality_failure;
    let aggregate_only = !aggregate_only_failure;
    let gates_open = request.policy_allow
        && request.protected_closure
        && request.signed_approval
        && locality_gate
        && aggregate_only;
    let disposition = if !gates_open {
        "blocked"
    } else if quorum >= request.minimum_quorum && request.budget_units >= required_budget {
        "ready"
    } else {
        "needs_refinement"
    };
    if request.budget_units < required_budget {
        omissions.insert("workbench:budget-exhausted".into());
    }
    if !request.policy_allow {
        omissions.insert("workbench:policy-denied".into());
    }
    if !request.protected_closure {
        omissions.insert("workbench:protected-closure-incomplete".into());
    }
    if !request.signed_approval {
        omissions.insert("workbench:signed-approval-missing".into());
    }
    if disposition == "ready" {
        actions.extend([
            "action:open-decision-section".to_string(),
            "action:export-digest-only-context".to_string(),
        ]);
    } else if disposition == "blocked" {
        blocked_actions.extend([
            "action:open-decision-section".to_string(),
            "action:export-digest-only-context".to_string(),
            "action:replay-local-federated-view".to_string(),
        ]);
        actions.clear();
        actions.insert("action:inspect-block-reason".into());
    } else {
        actions.extend([
            "action:review-peer-outcomes".to_string(),
            "action:request-federation-refinement".to_string(),
        ]);
        uncertainty.insert("workbench:quorum-not-admitted".into());
    }
    if !stale.is_empty() {
        views.insert("view:stale-peers".into());
    }
    if !unknown.is_empty() {
        views.insert("view:uncertain-peers".into());
    }
    if !blocked.is_empty() {
        views.insert("view:blocked-peers".into());
    }
    let institution_order = institutions.into_iter().collect::<Vec<_>>();
    let qualified_institution_order = qualified.into_iter().collect::<Vec<_>>();
    let stale_institution_order = stale.into_iter().collect::<Vec<_>>();
    let blocked_institution_order = blocked.into_iter().collect::<Vec<_>>();
    let unknown_institution_order = unknown.into_iter().collect::<Vec<_>>();
    let view_order = views.into_iter().collect::<Vec<_>>();
    let action_order = actions.into_iter().collect::<Vec<_>>();
    let blocked_action_order = blocked_actions.into_iter().collect::<Vec<_>>();
    let aggregate_order = aggregate.into_iter().collect::<Vec<_>>();
    let omissions = omissions.into_iter().collect::<Vec<_>>();
    let uncertainty = uncertainty.into_iter().collect::<Vec<_>>();
    let negative_evidence = negative.into_iter().collect::<Vec<_>>();
    let effect_receipts = if disposition == "blocked" {
        vec!["block:unsafe-release".into()]
    } else {
        vec![format!(
            "view:local-federated-context-workbench:{}",
            request.session_id
        )]
    };
    let raw_data_local = true;
    let checkpoint_digest = ContentHash::of_value(&json!({"session_id": request.session_id, "institution_order": institution_order, "qualified_institution_order": qualified_institution_order, "stale_institution_order": stale_institution_order, "blocked_institution_order": blocked_institution_order, "unknown_institution_order": unknown_institution_order, "view_order": view_order, "action_order": action_order, "blocked_action_order": blocked_action_order, "quorum": quorum, "minimum_quorum": request.minimum_quorum, "current_epoch": request.current_epoch, "budget_units": request.budget_units, "consumed_budget_units": consumed, "disposition": disposition, "replay_identity": request.replay_identity, "raw_data_local": raw_data_local, "aggregate_only": aggregate_only})).map_err(|error| FederatedContextWorkbenchError::Artifact(error.to_string()))?;
    let federation_envelope_digest = ContentHash::of_value(&json!({"federation_id": request.federation_id, "query_id": request.query_id, "goal": request.goal, "semantic_profile": request.semantic_profile, "aggregate_order": aggregate_order, "checkpoint_digest": checkpoint_digest, "replay_identity": request.replay_identity, "raw_data_local": raw_data_local, "aggregate_only": aggregate_only, "boundary": PRECLINICAL_BOUNDARY})).map_err(|error| FederatedContextWorkbenchError::Artifact(error.to_string()))?;
    let artifact_payload = json!({"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "contract_version": CONTRACT_VERSION, "feature_id": FEATURE_ID, "session_id": request.session_id, "federation_id": request.federation_id, "query_id": request.query_id, "goal": request.goal, "semantic_profile": request.semantic_profile, "disposition": disposition, "institution_order": institution_order, "qualified_institution_order": qualified_institution_order, "stale_institution_order": stale_institution_order, "blocked_institution_order": blocked_institution_order, "unknown_institution_order": unknown_institution_order, "view_order": view_order, "action_order": action_order, "blocked_action_order": blocked_action_order, "aggregate_order": aggregate_order, "quorum": quorum, "minimum_quorum": request.minimum_quorum, "current_epoch": request.current_epoch, "budget_units": request.budget_units, "consumed_budget_units": consumed, "checkpoint_digest": checkpoint_digest, "federation_envelope_digest": federation_envelope_digest, "replay_identity": request.replay_identity, "omissions": omissions, "uncertainty": uncertainty, "negative_evidence": negative_evidence, "effect_receipts": effect_receipts, "raw_data_local": raw_data_local, "aggregate_only": aggregate_only, "boundary": PRECLINICAL_BOUNDARY});
    let artifact = TypedResearchArtifact::from_payload(
        format!("brain-federated-context-workbench:{}", request.session_id),
        WORKBENCH_CONTENT_TYPE,
        &artifact_payload,
        Vec::new(),
        Vec::new(),
    )
    .map_err(|error| FederatedContextWorkbenchError::Artifact(error.to_string()))?;
    let receipt = FederatedContextWorkbenchReceipt {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        session_id: request.session_id.clone(),
        federation_id: request.federation_id.clone(),
        query_id: request.query_id.clone(),
        goal: request.goal.clone(),
        semantic_profile: request.semantic_profile.clone(),
        disposition: disposition.into(),
        institution_order,
        qualified_institution_order,
        stale_institution_order,
        blocked_institution_order,
        unknown_institution_order,
        view_order,
        action_order,
        blocked_action_order,
        aggregate_order,
        quorum,
        minimum_quorum: request.minimum_quorum,
        current_epoch: request.current_epoch,
        budget_units: request.budget_units,
        consumed_budget_units: consumed,
        checkpoint_digest,
        federation_envelope_digest,
        replay_identity: request.replay_identity.clone(),
        omissions,
        uncertainty,
        negative_evidence,
        effect_receipts,
        artifact,
        raw_data_local,
        aggregate_only,
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    receipt.validate()?;
    Ok(receipt)
}

fn identity_keys(values: &[String]) -> BTreeSet<String> {
    values
        .iter()
        .map(|value| value.to_ascii_lowercase())
        .collect()
}

fn validate_text(value: &str, field: &str) -> Result<(), FederatedContextWorkbenchError> {
    if value.trim().is_empty()
        || value != value.trim()
        || value.len() > MAX_TEXT_BYTES
        || value.chars().any(char::is_control)
    {
        return Err(FederatedContextWorkbenchError::Invalid(format!(
            "{field} must be bounded, non-empty text without padding or control characters"
        )));
    }
    Ok(())
}

fn validate_unique(values: &[String], field: &str) -> Result<(), FederatedContextWorkbenchError> {
    let mut seen = BTreeSet::new();
    for value in values {
        validate_text(value, field)?;
        if !seen.insert(value.to_ascii_lowercase()) {
            return Err(FederatedContextWorkbenchError::Invalid(format!(
                "{field} contains duplicate or case-colliding values"
            )));
        }
    }
    Ok(())
}

fn validate_sorted_unique(
    values: &[String],
    field: &str,
) -> Result<(), FederatedContextWorkbenchError> {
    validate_unique(values, field)?;
    if values.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(FederatedContextWorkbenchError::Invalid(format!(
            "{field} is not in canonical order"
        )));
    }
    Ok(())
}

fn receipt_payload(receipt: &FederatedContextWorkbenchReceipt) -> serde_json::Value {
    json!({
        "schema_version": receipt.schema_version,
        "contract_version": receipt.contract_version,
        "feature_id": receipt.feature_id,
        "session_id": receipt.session_id,
        "federation_id": receipt.federation_id,
        "query_id": receipt.query_id,
        "goal": receipt.goal,
        "semantic_profile": receipt.semantic_profile,
        "disposition": receipt.disposition,
        "institution_order": receipt.institution_order,
        "qualified_institution_order": receipt.qualified_institution_order,
        "stale_institution_order": receipt.stale_institution_order,
        "blocked_institution_order": receipt.blocked_institution_order,
        "unknown_institution_order": receipt.unknown_institution_order,
        "view_order": receipt.view_order,
        "action_order": receipt.action_order,
        "blocked_action_order": receipt.blocked_action_order,
        "aggregate_order": receipt.aggregate_order,
        "quorum": receipt.quorum,
        "minimum_quorum": receipt.minimum_quorum,
        "current_epoch": receipt.current_epoch,
        "budget_units": receipt.budget_units,
        "consumed_budget_units": receipt.consumed_budget_units,
        "checkpoint_digest": receipt.checkpoint_digest,
        "federation_envelope_digest": receipt.federation_envelope_digest,
        "replay_identity": receipt.replay_identity,
        "omissions": receipt.omissions,
        "uncertainty": receipt.uncertainty,
        "negative_evidence": receipt.negative_evidence,
        "effect_receipts": receipt.effect_receipts,
        "raw_data_local": receipt.raw_data_local,
        "aggregate_only": receipt.aggregate_only,
        "boundary": receipt.boundary,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn hash(value: &str) -> ContentHash {
        ContentHash::of_bytes(value.as_bytes())
    }
    fn request() -> FederatedContextWorkbenchRequest {
        let replay = hash("federated-workbench-replay");
        let peer = |id: &str| FederatedContextWorkbenchPeer {
            institution_id: id.into(),
            epoch: 10,
            semantic_profile: "profile:v1".into(),
            context_digest: replay.clone(),
            section_digest: replay.clone(),
            replay_identity: replay.clone(),
            evidence_state: EvidenceState::Supported,
            policy_allow: true,
            protected_closure: true,
            signed_approval: true,
            raw_data_local: true,
            aggregate_only: true,
            boundary: PRECLINICAL_BOUNDARY.into(),
        };
        FederatedContextWorkbenchRequest {
            session_id: "session:federated-workbench".into(),
            federation_id: "federation:preclinical".into(),
            query_id: "query:context".into(),
            goal: "review federated context".into(),
            semantic_profile: "profile:v1".into(),
            required_institution_ids: vec!["institution:a".into(), "institution:b".into()],
            peers: vec![peer("institution:a"), peer("institution:b")],
            minimum_quorum: 2,
            current_epoch: 10,
            max_epoch_lag: 1,
            budget_units: 2,
            replay_identity: replay,
            policy_allow: true,
            protected_closure: true,
            signed_approval: true,
            raw_data_local: true,
            aggregate_only: true,
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }
    #[test]
    fn manifest_is_a2_and_authorized() {
        assert_eq!(
            federated_context_workbench_manifest().autonomy_tier,
            AutonomyTier::A2
        );
        assert_eq!(
            federated_context_workbench_manifest()
                .authority_requirements
                .len(),
            1
        );
    }
    #[test]
    fn quorum_is_ready() {
        let receipt = render_federated_context_workbench(&request()).unwrap();
        assert_eq!(receipt.disposition, "ready");
        assert_eq!(receipt.quorum, 2);
        assert_eq!(receipt.aggregate_order.len(), 2);
    }
    #[test]
    fn stale_peer_is_explicit() {
        let mut value = request();
        value.peers[1].epoch = 1;
        let receipt = render_federated_context_workbench(&value).unwrap();
        assert!(receipt
            .stale_institution_order
            .contains(&"institution:b".into()));
        assert_eq!(receipt.disposition, "needs_refinement");
    }
    #[test]
    fn semantic_mismatch_is_negative() {
        let mut value = request();
        value.peers[0].semantic_profile = "profile:other".into();
        let receipt = render_federated_context_workbench(&value).unwrap();
        assert!(receipt
            .negative_evidence
            .iter()
            .any(|item| item.contains("semantic-profile-mismatch")));
    }
    #[test]
    fn policy_denial_blocks_actions() {
        let mut value = request();
        value.policy_allow = false;
        let receipt = render_federated_context_workbench(&value).unwrap();
        assert_eq!(receipt.disposition, "blocked");
        assert_eq!(receipt.effect_receipts, vec!["block:unsafe-release"]);
    }
    #[test]
    fn digest_is_stable() {
        let receipt = render_federated_context_workbench(&request()).unwrap();
        assert_eq!(receipt.digest().unwrap(), receipt.digest().unwrap());
    }
    #[test]
    fn institution_count_cannot_overflow_receipt_quorum() {
        let mut value = request();
        value.required_institution_ids = (0..=usize::from(u16::MAX))
            .map(|index| format!("institution:{index}"))
            .collect();
        assert!(matches!(
            render_federated_context_workbench(&value),
            Err(FederatedContextWorkbenchError::Invalid(_))
        ));
    }
    #[test]
    fn peer_locality_failure_is_blocked_and_retained() {
        let mut value = request();
        value.peers[0].raw_data_local = false;
        let receipt = render_federated_context_workbench(&value).unwrap();
        assert_eq!(receipt.disposition, "blocked");
        assert!(receipt.raw_data_local);
        assert!(receipt
            .omissions
            .iter()
            .any(|item| item == "workbench:raw-data-locality-failed"));
        assert!(receipt.validate().is_ok());
    }
    #[test]
    fn non_aggregate_peer_is_blocked_and_retained() {
        let mut value = request();
        value.peers[0].aggregate_only = false;
        let receipt = render_federated_context_workbench(&value).unwrap();
        assert_eq!(receipt.disposition, "blocked");
        assert!(!receipt.aggregate_only);
        assert!(receipt
            .omissions
            .iter()
            .any(|item| item == "workbench:aggregate-only-required"));
        assert!(receipt.validate().is_ok());
    }
    #[test]
    fn workbench_artifact_payload_is_bound() {
        let mut receipt = render_federated_context_workbench(&request()).unwrap();
        receipt.goal = "tampered goal".into();
        assert!(receipt.validate().is_err());
    }
}
